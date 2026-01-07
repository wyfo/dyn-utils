//! A utility library for working with [trait objects].
//!
//! Trait objects (i.e. `dyn Trait`) are unsized and therefore need to be stored in a container
//! such as `Box`. This crate provides [`DynObject`], a container for trait objects with a
//! generic [`storage`].
//!
//! [`storage::Raw`] stores objects in place, making `DynObject<dyn Trait, storage::Raw>`
//! allocation-free. On the other hand, [`storage::RawOrBox`] falls back to an allocated `Box` if
//! the object is too large to fit in place.
//!
//! Avoiding one allocation makes `DynObject` a good alternative to `Box` when writing a
//! [dyn-compatible] version of a trait with return-position `impl Trait`, such as async methods.
//!
//! # Examples
//!
//! ```rust
//! use dyn_utils::object::DynObject;
//!
//! trait Callback {
//!     fn call(&self, arg: &str) -> impl Future<Output = ()> + Send;
//! }
//!
//! // Dyn-compatible version
//! trait DynCallback {
//!     fn call<'a>(&'a self, arg: &'a str) -> DynObject<dyn Future<Output = ()> + Send + 'a>;
//! }
//!
//! impl<T: Callback> DynCallback for T {
//!     fn call<'a>(&'a self, arg: &'a str) -> DynObject<dyn Future<Output = ()> + Send + 'a> {
//!         DynObject::new(self.call(arg))
//!     }
//! }
//!
//! async fn exec_callback(callback: &dyn DynCallback) {
//!     callback.call("Hello world!").await;
//! }
//! ```
//!
//! This crate also provides [`dyn_trait`] proc-macro to achieve the same result as above:
//!
//! ```rust
//! # #[cfg(feature = "macros")]
//! #[dyn_utils::dyn_trait] // generates `DynCallback` trait
//! trait Callback {
//!     fn call(&self, arg: &str) -> impl Future<Output = ()> + Send;
//! }
//!
//! # #[cfg(feature = "macros")]
//! async fn exec_callback(callback: &dyn DynCallback) {
//!     callback.call("Hello world!").await;
//! }
//! ```
//!
//! [trait objects]: https://doc.rust-lang.org/std/keyword.dyn.html
//! [dyn-compatible]: https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![no_std]
#![forbid(missing_docs)]

#[cfg(any(feature = "alloc", doc))]
extern crate alloc;

use core::{
    hint, mem,
    pin::Pin,
    task::{Context, Poll},
};

mod impls;
#[cfg(feature = "macros")]
mod macros;
pub mod object;
pub mod storage;

#[cfg(feature = "macros")]
pub use macros::{dyn_object, dyn_trait, sync};
pub use object::DynObject;
#[cfg(all(doc, not(feature = "macros")))]
#[doc(hidden)]
pub fn dyn_trait() {}
#[cfg(all(doc, not(feature = "macros")))]
#[doc(hidden)]
pub fn dyn_object() {}

/// An async wrapper with an optimized synchronous execution path.
///
/// It is used in combination with `Future` trait objects, such as
/// `DynObject<dyn Future<Output=T>>`.
///
/// # Examples
///
/// ```rust
/// # use dyn_utils::{DynObject, TrySync};
///
/// trait Callback {
///     fn call(&self, arg: &str) -> TrySync<DynObject<dyn Future<Output = ()>>>;
/// }
///
/// struct Print;
/// impl Callback for Print {
///     fn call(&self, arg: &str) -> TrySync<DynObject<dyn Future<Output = ()>>> {
///         println!("{arg}");
///         TrySync::Sync(())
///     }
/// }
/// ```
pub enum TrySync<F: Future> {
    /// Optimized synchronous execution path.
    Sync(F::Output),
    /// Asynchronous wrapper
    Async(F),
    /// Synchronous execution path already polled
    SyncPolled,
}

impl<F: Future> TrySync<F> {
    /// # Safety
    ///
    /// `self` must be `Self::Sync` variant.
    #[cfg_attr(coverage_nightly, coverage(off))] // Because of `unreachable_unchecked` branch
    #[inline(always)]
    unsafe fn take_sync(&mut self) -> F::Output {
        match mem::replace(self, Self::SyncPolled) {
            Self::Sync(res) => res,
            // SAFETY: as per function contract
            _ => unsafe { hint::unreachable_unchecked() },
        }
    }
}

impl<F: Future> Future for TrySync<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: pinned data is not moved
        match unsafe { self.get_unchecked_mut() } {
            // SAFETY: res is `Self::Sync`
            res @ TrySync::Sync(_) => Poll::Ready(unsafe { res.take_sync() }),
            // SAFETY: `fut` is pinned as `self` is
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
