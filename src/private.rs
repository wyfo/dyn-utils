use core::{alloc::Layout, ptr::NonNull};

use crate::storage::Storage;

pub trait DynTrait {
    type VTable: 'static;
    fn drop_in_place(vtable: &Self::VTable) -> Option<unsafe fn(NonNull<()>)>;
    fn layout(vtable: &Self::VTable) -> Layout;
}

pub unsafe trait VTable<T>: DynTrait {
    fn vtable<S: Storage>() -> &'static Self::VTable;
}
