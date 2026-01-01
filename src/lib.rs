#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![no_std]
// #![forbid(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{alloc::Layout, ptr::NonNull};

#[doc(hidden)]
pub mod private;
pub mod storage;

pub use dyn_utils_macros::*;
pub use elain::*;

/// Default storage for methods' return.
pub type DefaultStorage = storage::RawOrBox<{ 16 * size_of::<usize>() }>;

/// A storage that can be used to store dynamic type-erased objects.
///
/// # Safety
///
/// `can_store` return must be constant for `T`.
/// `ptr`/`ptr_mut` must return a pointer to the data stored in the storage.
pub unsafe trait Storage: Sized + 'static {
    fn new<T>(data: T) -> Self;
    fn ptr(&self) -> NonNull<()>;
    fn ptr_mut(&mut self) -> NonNull<()>;
    unsafe fn as_ref<T>(&self) -> &T {
        unsafe { self.ptr().cast().as_ref() }
    }
    unsafe fn as_mut<T>(&mut self) -> &mut T {
        unsafe { self.ptr().cast().as_mut() }
    }
    /// # Safety
    ///
    /// `drop_in_place` must be called once, and the storage must not be used
    /// after. `layout` must be the layout of the `data` passed in `Self::new`
    /// (or in other constructor like `new_box`, etc.)
    unsafe fn drop_in_place(&mut self, layout: Layout);
}
