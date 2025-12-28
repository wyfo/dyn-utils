use core::{alloc::Layout, marker::PhantomData, mem, ptr::NonNull};

use crate::Storage;

#[derive(Debug)]
pub struct DynVTable {
    drop_in_place: Option<unsafe fn(NonNull<()>)>,
    layout: Layout,
}

impl StorageVTable for DynVTable {
    type DynTrait = ();
    fn drop_in_place(&self) -> Option<unsafe fn(NonNull<()>)> {
        self.drop_in_place
    }
    fn layout(&self) -> Layout {
        self.layout
    }
}

impl DynVTable {
    #[cfg_attr(coverage_nightly, coverage(off))] // const fn
    pub const fn new<S: Storage, T>() -> Self {
        Self {
            drop_in_place: const {
                if mem::needs_drop::<T>() {
                    Some(|ptr_mut| unsafe { ptr_mut.cast::<T>().drop_in_place() })
                } else {
                    None
                }
            },
            layout: const { Layout::new::<T>() },
        }
    }
}

pub trait StorageVTable: 'static {
    type DynTrait: ?Sized;
    fn drop_in_place(&self) -> Option<unsafe fn(NonNull<()>)>;
    fn layout(&self) -> Layout;
}

pub struct StorageMoved<'a, S: Storage, T> {
    storage: &'a mut S,
    _phantom: PhantomData<T>,
}

impl<'a, S: Storage, T> StorageMoved<'a, S, T> {
    /// # Safety
    ///
    /// `storage` must have been instantiated with type `T`.
    /// `storage` must neither be accessed, nor dropped, after `StorageMoved` instantiation.
    pub unsafe fn new(storage: &'a mut S) -> Self {
        Self {
            storage,
            _phantom: PhantomData,
        }
    }

    /// # Safety
    ///
    /// `read` must be called only once.
    pub unsafe fn read(&self) -> T {
        // SAFETY: `storage` stores a `T`
        unsafe { self.storage.ptr().cast().read() }
    }
}

impl<S: Storage, T> Drop for StorageMoved<'_, S, T> {
    fn drop(&mut self) {
        // SAFETY: the storage data is no longer accessed after the call,
        // and is matched by the vtable as per function contract, as per
        // `Self::new` contract
        unsafe { self.storage.drop_in_place(Layout::new::<T>()) }
    }
}
