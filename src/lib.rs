//! A utility library for working with [trait objects].
//!
//! Trait objects, i.e. `dyn Trait`, are unsized and requires to be stored in a container
//! like `Box`. This crate provides [`DynObject`], a container for trait object with a
//! generic [`storage`].
//!
//! [`storage::Raw`] stores object in place, making `DynObject<dyn Trait, storage::Raw>`
//! allocation-free. On the other hand, [`storage::RawOrBox`], used in [`DefaultStorage`],
//! falls back to an allocated `Box` if  the object is too big to fit in place.
//! <br>
//! These storages thus makes `DynObject` a good alternative to `Box` when it comes to write a
//! [dyn-compatible] version of a trait with return-position `impl Trait`, such as async methods.
//!
//! # Examples
//!
//! ```rust
//! trait Callback {
//!     fn call(&self, arg: &str) -> impl Future<Output = ()> + Send;
//! }
//!
//! // Dyn-compatible version
//! trait DynCallback {
//!     fn call<'a>(&'a self, arg: &'a str) -> DynObject<dyn Future<Output = ()> + Send + 'a>;
//! }
//!
//! impl<T: Callback> DynCallback for T {
//!     fn call<'a>(&'a self, arg: &'a str) -> DynObject<dyn Future<Output = ()> + Send + 'a> {
//!         DynObject::new(self.call(arg))
//!     }
//! }
//!
//! async fn exec_callback(callback: &dyn DynCallback) {
//!     callback.call("Hello world!").await;
//! }
//! ```
//!
//! This crate also provides [`dyn_trait`] proc-macro to do the same as above:
//!
//! ```rust
//! #[dyn_utils::dyn_trait] // generates `DynCallback` trait
//! trait Callback {
//!     fn call(&self, arg: &str) -> impl Future<Output = ()> + Send;
//! }
//!
//! async fn exec_callback(callback: &dyn DynCallback) {
//!     callback.call("Hello world!").await;
//! }
//! ```
//!
//! [trait objects]: https://doc.rust-lang.org/std/keyword.dyn.html
//! [dyn-compatible]: https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![no_std]
#![forbid(missing_docs)]

#[cfg(any(feature = "alloc", doc))]
extern crate alloc;

use core::{
    any::{Any, TypeId},
    fmt,
    marker::PhantomData,
    pin::Pin,
};

use crate::{impls::any_impl, storage::Storage};

mod impls;
#[doc(hidden)]
pub mod private;
pub mod storage;

pub use dyn_utils_macros::*;

/// Default storage for [`DynObject`], and used in [`dyn_trait`] macro.
pub type DefaultStorage = storage::RawOrBox<{ 128 * size_of::<usize>() }>;

/// A trait object whose data is stored in a generic [`Storage`].
///
/// [`dyn_object`] proc-macro can be used to make a trait compatible with `DynObject`.
///
/// # Examples
///
/// ```rust
/// # use dyn_utils::DynObject;
/// let future: DynObject<dyn Future<Output = usize>> = DynObject::new(async { 42 });
/// # futures::executor::block_on(async move {
/// assert_eq!(future.await, 42);
/// # });
/// ```
pub struct DynObject<Dyn: private::DynTrait + ?Sized, S: Storage = DefaultStorage> {
    storage: S,
    vtable: &'static Dyn::VTable,
    _phantom: PhantomData<Dyn>,
}

// SAFETY: DynObject is just a wrapper around `Dyn`
unsafe impl<Dyn: Send + private::DynTrait + ?Sized, S: Storage> Send for DynObject<Dyn, S> {}
// SAFETY: DynObject is just a wrapper around `Dyn`
unsafe impl<Dyn: Sync + private::DynTrait + ?Sized, S: Storage> Sync for DynObject<Dyn, S> {}
impl<Dyn: Unpin + private::DynTrait + ?Sized, S: Storage> Unpin for DynObject<Dyn, S> {}

impl<S: Storage, Dyn: private::DynTrait + ?Sized> DynObject<Dyn, S> {
    /// Constructs a new `DynObject` from an object implementing the trait
    pub fn new<T>(object: T) -> Self
    where
        Dyn: private::VTable<T>,
    {
        Self {
            storage: S::new(object),
            vtable: Dyn::vtable::<S>(),
            _phantom: PhantomData,
        }
    }

    /// Construct a new `DynObject` from a boxed object implementing the trait
    #[cfg(feature = "alloc")]
    pub fn from_box<T>(boxed: alloc::boxed::Box<T>) -> Self
    where
        S: storage::FromBox,
        Dyn: private::VTable<T>,
    {
        Self {
            storage: S::from_box(boxed),
            vtable: Dyn::vtable::<S>(),
            _phantom: PhantomData,
        }
    }

    #[doc(hidden)]
    pub fn vtable(&self) -> &'static Dyn::VTable {
        self.vtable
    }

    #[doc(hidden)]
    pub fn storage(&self) -> &S {
        &self.storage
    }

    #[doc(hidden)]
    pub fn storage_mut(&mut self) -> &mut S {
        &mut self.storage
    }

    #[doc(hidden)]
    pub fn storage_pinned_mut(self: Pin<&mut Self>) -> Pin<&mut S> {
        // SAFETY: `self.storage` is structurally pinned
        unsafe { self.map_unchecked_mut(|this| &mut this.storage) }
    }

    #[doc(hidden)]
    pub fn insert<T>(this: &mut Option<Self>, object: T) -> &mut T
    where
        Dyn: private::VTable<T>,
    {
        let storage = this.insert(DynObject::new(object));
        // SAFETY: storage has been initialized with `T`
        unsafe { storage.storage_mut().as_mut::<T>() }
    }

    #[doc(hidden)]
    pub fn insert_pinned<T>(this: Pin<&mut Option<Self>>, object: T) -> Pin<&mut T>
    where
        Dyn: private::VTable<T>,
    {
        // SAFETY: the returned reference cannot is structurally pinned
        unsafe { this.map_unchecked_mut(|opt| Self::insert(opt, object)) }
    }
}

impl<Dyn: private::DynTrait + ?Sized, S: Storage> Drop for DynObject<Dyn, S> {
    fn drop(&mut self) {
        if let Some(drop_inner) = Dyn::drop_in_place(self.vtable) {
            // SAFETY: the storage data is no longer accessed after the call,
            // and is matched by the vtable as per function contract.
            unsafe { drop_inner(self.storage.ptr_mut()) };
        }
        let layout = Dyn::layout(self.vtable);
        // SAFETY: the storage data is no longer accessed after the call,
        // and is matched by the vtable as per function contract.
        unsafe { self.storage.drop_in_place(layout) };
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<Dyn: private::DynTrait<VTable: fmt::Debug> + ?Sized, S: Storage + fmt::Debug> fmt::Debug
    for DynObject<Dyn, S>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynObject")
            .field("inner", &self.storage)
            .field("vtable", &self.vtable)
            .finish()
    }
}

// Putting this in impls module make these methods appears before others,
// so it has to be explicitly put after other methods
any_impl!(dyn Any);
any_impl!(dyn Any + Send);
any_impl!(dyn Any + Send + Sync);

#[cfg_attr(coverage_nightly, coverage(off))] // Because of `unreachable_unchecked` branch
#[cfg(test)]
mod tests {
    use core::any::Any;

    use crate::impls::any_test;

    any_test!(dyn_any, dyn Any);
    any_test!(dyn_any_send, dyn Any + Send);
    any_test!(dyn_any_send_sync, dyn Any + Send + Sync);
}
