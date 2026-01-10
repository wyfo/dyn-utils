//! [`DynObject`] implementation.
use core::{alloc::Layout, any::Any, fmt, marker::PhantomData, mem, pin::Pin, ptr::NonNull};

use crate::{
    impls::any_impl,
    storage::{DefaultStorage, Storage},
};

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
///
/// [`dyn_object`]: crate::dyn_object
pub struct DynObject<Dyn: DynTrait + ?Sized, S: Storage = DefaultStorage> {
    storage: S,
    vtable: &'static Dyn::Vtable,
    _phantom: PhantomData<Dyn>,
}

// SAFETY: DynObject is just a wrapper around `Dyn`
unsafe impl<Dyn: Send + DynTrait + ?Sized, S: Storage> Send for DynObject<Dyn, S> {}

// SAFETY: DynObject is just a wrapper around `Dyn`
unsafe impl<Dyn: Sync + DynTrait + ?Sized, S: Storage> Sync for DynObject<Dyn, S> {}

impl<Dyn: Unpin + DynTrait + ?Sized, S: Storage> Unpin for DynObject<Dyn, S> {}

impl<S: Storage, Dyn: DynTrait + ?Sized> DynObject<Dyn, S> {
    /// Constructs a new `DynObject` from an object implementing the trait
    pub fn new<T>(object: T) -> Self
    where
        Dyn: Vtable<T>,
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
        S: crate::storage::FromBox,
        Dyn: Vtable<T>,
    {
        Self {
            storage: S::from_box(boxed),
            vtable: Dyn::vtable::<S>(),
            _phantom: PhantomData,
        }
    }

    #[doc(hidden)]
    pub fn vtable(&self) -> &'static Dyn::Vtable {
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
        Dyn: Vtable<T>,
    {
        let storage = this.insert(DynObject::new(object));
        // SAFETY: storage has been initialized with `T`
        unsafe { storage.storage_mut().as_mut::<T>() }
    }

    #[doc(hidden)]
    pub fn insert_pinned<T>(this: Pin<&mut Option<Self>>, object: T) -> Pin<&mut T>
    where
        Dyn: Vtable<T>,
    {
        // SAFETY: the returned reference cannot is structurally pinned
        unsafe { this.map_unchecked_mut(|opt| Self::insert(opt, object)) }
    }
}

impl<Dyn: DynTrait + ?Sized, S: Storage> Drop for DynObject<Dyn, S> {
    fn drop(&mut self) {
        if let Some(drop_inner) = Dyn::drop_in_place_fn(self.vtable) {
            // SAFETY: the storage data is no longer accessed after the call,
            // and is matched by the vtable as per function contract.
            unsafe { drop_inner(self.storage_mut().ptr_mut()) };
        }
        let layout = Dyn::layout(self.vtable);
        // SAFETY: the storage data is no longer accessed after the call,
        // and is matched by the vtable as per function contract.
        unsafe { self.storage_mut().drop_in_place(layout) };
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<Dyn: DynTrait<Vtable: fmt::Debug> + ?Sized, S: Storage + fmt::Debug> fmt::Debug
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

/// A trait object with its associated vtable.
pub trait DynTrait {
    /// The trait object vtable.
    type Vtable: 'static;
    /// Returns the drop function of the trait object as stored in vtable.
    fn drop_in_place_fn(vtable: &Self::Vtable) -> Option<unsafe fn(NonNull<()>)>;
    /// Returns the layout of the trait object as stored in vtable.
    fn layout(vtable: &Self::Vtable) -> Layout;
}

/// A vtable constructor.
///
/// # Safety
///
/// - `DynTrait::layout` must return `core::alloc::Layout::new::<T>()`.
/// - `DynTrait::drop_in_place_fn` must returns
///   `<Self as crate::object::Vtable<T>>::DROP_IN_PLACE_FN`
pub unsafe trait Vtable<T>: DynTrait {
    /// Returns the vtable for a given `T` stored in `S`.
    fn vtable<S: Storage>() -> &'static Self::Vtable;
    /// The function has the same safety contract that [`core::ptr::drop_in_place`].
    const DROP_IN_PLACE_FN: Option<unsafe fn(NonNull<()>)> = if mem::needs_drop::<T>() {
        // SAFETY: as per function contract
        Some(|ptr_mut| unsafe { ptr_mut.cast::<T>().drop_in_place() })
    } else {
        None
    };
}

#[cfg(test)]
mod tests {
    use crate::{impls::any_test, object::Any};

    any_test!(dyn_any, dyn Any);
    any_test!(dyn_any_send, dyn Any + Send);
    any_test!(dyn_any_send_sync, dyn Any + Send + Sync);
}
