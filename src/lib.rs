#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{
    alloc::Layout,
    hint, mem,
    pin::Pin,
    ptr::NonNull,
    task::{Context, Poll},
};

#[doc(hidden)]
pub mod private;
pub mod storage;
mod vtables;

pub use dyn_utils_macros::*;
pub use elain::*;
pub use storage::DynStorage;

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
        unsafe { self.ptr_mut().cast().as_mut() }
    }
    unsafe fn as_pinned_mut<T>(self: Pin<&mut Self>) -> Pin<&mut T> {
        unsafe { self.map_unchecked_mut(|this| this.as_mut()) }
    }
    /// # Safety
    ///
    /// `drop_in_place` must be called once, and the storage must not be used
    /// after. `layout` must be the layout of the `data` passed in `Self::new`
    /// (or in other constructor like `new_box`, etc.)
    unsafe fn drop_in_place(&mut self, layout: Layout);
}

pub enum TrySync<F: Future> {
    Sync(F::Output),
    Async(F),
    SyncPolled,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<F: Future> Future for TrySync<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match unsafe { self.get_unchecked_mut() } {
            res @ TrySync::Sync(_) => match mem::replace(res, TrySync::SyncPolled) {
                TrySync::Sync(res) => Poll::Ready(res),
                _ => unsafe { hint::unreachable_unchecked() },
            },
            TrySync::Async(fut) => unsafe { Pin::new_unchecked(fut) }.poll(cx),
            _ => panic!("future polled after completion"),
        }
    }
}
