use core::{alloc::Layout, marker::PhantomData, mem, pin::Pin, ptr::NonNull};

use crate::{storage::DynStorage, Storage};

pub trait StorageVTable: 'static {
    type DynTrait: ?Sized;
    fn drop_in_place(&self) -> Option<unsafe fn(NonNull<()>)>;
    fn layout(&self) -> Layout;
}

#[derive(Debug)]
pub struct DynVTable {
    drop_in_place: Option<unsafe fn(NonNull<()>)>,
    layout: Layout,
}

impl DynVTable {
    #[cfg_attr(coverage_nightly, coverage(off))] // const fn
    pub const fn new<S: Storage, __Dyn>() -> Self {
        Self {
            drop_in_place: const {
                if mem::needs_drop::<__Dyn>() {
                    Some(|ptr_mut| unsafe { ptr_mut.cast::<__Dyn>().drop_in_place() })
                } else {
                    None
                }
            },
            layout: const { Layout::new::<__Dyn>() },
        }
    }
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

pub unsafe fn insert_into_storage<'a, S: Storage, T: ?Sized, __Dyn>(
    __dyn: __Dyn,
    __storage: &'a mut Option<DynStorage<S, DynVTable, T>>,
) -> &'a mut __Dyn {
    let storage = __storage.insert(unsafe {
        DynStorage::from_raw_parts(S::new(__dyn), &const { DynVTable::new::<S, __Dyn>() })
    });
    unsafe { storage.inner_mut().ptr_mut().cast().as_mut() }
}

pub unsafe fn insert_into_storage_pinned<'a, S: Storage, T: ?Sized, __Dyn>(
    __dyn: __Dyn,
    __storage: Pin<&'a mut Option<DynStorage<S, DynVTable, T>>>,
) -> Pin<&'a mut __Dyn> {
    unsafe { __storage.map_unchecked_mut(|s| insert_into_storage(__dyn, s)) }
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
