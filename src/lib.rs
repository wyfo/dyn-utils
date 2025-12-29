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

#[cfg(not(all(feature = "alloc", feature = "either")))]
/// Default storage for methods' return.
pub type DefaultStorage = storage::Raw<{ 16 * size_of::<usize>() }>;
#[cfg(all(feature = "alloc", feature = "either"))]
/// Default storage for methods' return.
pub type DefaultStorage = either::Either<storage::Raw<{ 16 * size_of::<usize>() }>, storage::Box>;

/// A storage that can be used to store dynamic type-erased objects.
///
/// # Safety
///
/// `can_store` return must be constant for `T`.
/// `ptr`/`ptr_mut` must return a pointer to the data stored in the storage.
pub unsafe trait Storage: Sized + 'static {
    fn try_new<T>(data: T) -> Result<Self, T>;
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

// SAFETY: Both `Raw` and `Box` implements `Storage`
// This enum is generic and the variant is chosen according constant predicate,
// so it's not possible to cover all variant for a specific monomorphization.
// https://github.com/taiki-e/cargo-llvm-cov/issues/394
#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(feature = "either")]
unsafe impl<S1: Storage, S2: Storage> Storage for either::Either<S1, S2> {
    #[inline(always)]
    fn try_new<T>(data: T) -> Result<Self, T> {
        Ok(match S1::try_new(data) {
            Ok(s) => either::Either::Left(s),
            Err(data) => either::Either::Right(S2::try_new(data)?),
        })
    }
    #[inline(always)]
    fn new<T>(data: T) -> Self {
        match S1::try_new(data) {
            Ok(s) => either::Either::Left(s),
            Err(data) => either::Either::Right(S2::new(data)),
        }
    }
    #[inline(always)]
    fn ptr(&self) -> NonNull<()> {
        match self {
            either::Either::Left(s) => s.ptr(),
            either::Either::Right(s) => s.ptr(),
        }
    }
    #[inline(always)]
    fn ptr_mut(&mut self) -> NonNull<()> {
        match self {
            either::Either::Left(s) => s.ptr_mut(),
            either::Either::Right(s) => s.ptr_mut(),
        }
    }
    #[inline(always)]
    unsafe fn drop_in_place(&mut self, layout: Layout) {
        match self {
            either::Either::Left(s) => unsafe { s.drop_in_place(layout) },
            either::Either::Right(s) => unsafe { s.drop_in_place(layout) },
        }
    }
}
