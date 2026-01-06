#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{
    fmt, hint,
    marker::PhantomData,
    mem,
    pin::Pin,
    task::{Context, Poll},
};

use storage::Storage;

mod impls;
#[doc(hidden)]
pub mod private;
pub mod storage;

pub use dyn_utils_macros::*;
pub use elain::*;

/// Default storage for [`DynObject`], and used in [`dyn_trait`] macro.
pub type DefaultStorage = storage::RawOrBox<{ 128 * size_of::<usize>() }>;

/// A trait object whose data is stored in a generic [`Storage`].
pub struct DynObject<Dyn: private::DynTrait + ?Sized, S: Storage = DefaultStorage> {
    storage: S,
    vtable: &'static Dyn::VTable,
    _phantom: PhantomData<Dyn>,
}

unsafe impl<Dyn: Send + private::DynTrait + ?Sized, S: Storage> Send for DynObject<Dyn, S> {}
unsafe impl<Dyn: Sync + private::DynTrait + ?Sized, S: Storage> Sync for DynObject<Dyn, S> {}
impl<Dyn: Unpin + private::DynTrait + ?Sized, S: Storage> Unpin for DynObject<Dyn, S> {}

impl<S: Storage, Dyn: private::DynTrait + ?Sized> DynObject<Dyn, S> {
    pub fn new<T>(data: T) -> Self
    where
        Dyn: private::VTable<T>,
    {
        Self {
            storage: S::new(data),
            vtable: Dyn::vtable::<S>(),
            _phantom: PhantomData,
        }
    }

    #[cfg(feature = "alloc")]
    pub fn from_box<T>(data: alloc::boxed::Box<T>) -> Self
    where
        S: storage::FromBox,
        Dyn: private::VTable<T>,
    {
        Self {
            storage: S::from_box(data),
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
        unsafe { self.map_unchecked_mut(|this| &mut this.storage) }
    }

    pub fn insert<T>(storage: &mut Option<Self>, data: T) -> &mut T
    where
        Dyn: private::VTable<T>,
    {
        let storage = storage.insert(DynObject::new(data));
        unsafe { storage.storage_mut().as_mut::<T>() }
    }

    pub fn insert_pinned<T>(storage: Pin<&mut Option<Self>>, data: T) -> Pin<&mut T>
    where
        Dyn: private::VTable<T>,
    {
        unsafe { storage.map_unchecked_mut(|s| Self::insert(s, data)) }
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

pub enum TrySync<F: Future> {
    Sync(F::Output),
    Async(F),
    SyncPolled,
}

impl<F: Future> TrySync<F> {
    #[cfg_attr(coverage_nightly, coverage(off))] // Because of `unreachable_unchecked` branch
    #[inline(always)]
    unsafe fn take_sync(&mut self) -> F::Output {
        match mem::replace(self, TrySync::SyncPolled) {
            TrySync::Sync(res) => res,
            _ => unsafe { hint::unreachable_unchecked() },
        }
    }
}

impl<F: Future> Future for TrySync<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match unsafe { self.get_unchecked_mut() } {
            res @ TrySync::Sync(_) => Poll::Ready(unsafe { res.take_sync() }),
            TrySync::Async(fut) => unsafe { Pin::new_unchecked(fut) }.poll(cx),
            _ => panic!("future polled after completion"),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))] // Because of `unreachable_unchecked` branch
#[cfg(test)]
mod tests {
    use core::{
        future::{Ready, ready},
        pin::pin,
    };

    use futures::FutureExt;

    use crate::TrySync;

    #[test]
    fn try_sync() {
        for try_sync in [TrySync::Sync(42), TrySync::Async(ready(42))] {
            assert_eq!(try_sync.now_or_never(), Some(42));
        }
    }

    #[test]
    #[should_panic(expected = "future polled after completion")]
    fn try_sync_polled_after_completion() {
        let mut try_sync = pin!(TrySync::<Ready<i32>>::Sync(42));
        assert_eq!(try_sync.as_mut().now_or_never(), Some(42));
        try_sync.now_or_never();
    }
}
