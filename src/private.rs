use core::{alloc::Layout, mem, ptr::NonNull};

use crate::storage::Storage;

pub trait DynTrait {
    type VTable: StorageVTable;
}

pub trait StorageVTable: 'static {
    fn dyn_vtable(&self) -> &DynVTable;
    fn drop_in_place(&self) -> Option<unsafe fn(NonNull<()>)> {
        self.dyn_vtable().drop_in_place
    }
    fn layout(&self) -> Layout {
        self.dyn_vtable().layout
    }
}

pub unsafe trait NewVTable<T>: DynTrait {
    fn new_vtable<S: Storage>() -> &'static Self::VTable;
}

#[derive(Debug)]
pub struct DynVTable {
    drop_in_place: Option<unsafe fn(NonNull<()>)>,
    layout: Layout,
}

impl DynVTable {
    #[cfg_attr(coverage_nightly, coverage(off))] // const fn
    pub const fn new<T>() -> Self {
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
