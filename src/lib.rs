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
/// `ptr`/`ptr_mut` must return a pointer to the data stored in the storage.
pub unsafe trait Storage: Sized + 'static {
    fn can_store<T>() -> bool;
    fn new<T>(data: T) -> Self;
    fn ptr(&self) -> NonNull<()>;
    fn ptr_mut(&mut self) -> NonNull<()>;
    /// # Safety
    ///
    /// `drop_in_place` must be called once, and the storage must not be used
    /// after. `layout` must be the layout of the `data` passed in `Self::new`
    /// (or in other constructor like `new_box`, etc.)
    unsafe fn drop_in_place(&mut self, layout: Layout);
}

#[cfg(feature = "either")]
unsafe impl<S1: Storage, S2: Storage> Storage for either::Either<S1, S2> {
    fn can_store<T>() -> bool {
        S1::can_store::<T>() || S2::can_store::<T>()
    }
    fn new<T>(data: T) -> Self {
        if S1::can_store::<T>() {
            either::Either::Left(S1::new(data))
        } else {
            either::Either::Right(S2::new(data))
        }
    }
    fn ptr(&self) -> NonNull<()> {
        match self {
            either::Either::Left(s) => s.ptr(),
            either::Either::Right(s) => s.ptr(),
        }
    }
    fn ptr_mut(&mut self) -> NonNull<()> {
        match self {
            either::Either::Left(s) => s.ptr_mut(),
            either::Either::Right(s) => s.ptr_mut(),
        }
    }
    unsafe fn drop_in_place(&mut self, layout: Layout) {
        match self {
            either::Either::Left(s) => unsafe { s.drop_in_place(layout) },
            either::Either::Right(s) => unsafe { s.drop_in_place(layout) },
        }
    }
}
