#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{
    fmt, hint, mem,
    pin::Pin,
    task::{Context, Poll},
};

use storage::Storage;

#[doc(hidden)]
pub mod private;
pub mod storage;
mod vtables;

pub use dyn_utils_macros::*;
pub use elain::*;

/// Default storage for return-position `impl Trait`.
pub type DefaultStorage = storage::RawOrBox<{ 16 * size_of::<usize>() }>;

pub struct DynStorage<Dyn: private::DynTrait + ?Sized, S: Storage = DefaultStorage> {
    inner: S,
    vtable: &'static Dyn::VTable,
}

unsafe impl<Dyn: private::DynTrait + ?Sized, S: Storage> Send for DynStorage<Dyn, S> {}

unsafe impl<Dyn: private::DynTrait + ?Sized, S: Storage> Sync for DynStorage<Dyn, S> {}

impl<S: Storage, Dyn: private::DynTrait + ?Sized> DynStorage<Dyn, S> {
    pub fn new<T>(data: T) -> Self
    where
        Dyn: private::NewVTable<T>,
    {
        Self {
            inner: S::new(data),
            vtable: Dyn::new_vtable::<S>(),
        }
    }
    #[doc(hidden)]
    pub fn vtable(&self) -> &'static Dyn::VTable {
        self.vtable
    }

    #[doc(hidden)]
    pub fn inner(&self) -> &S {
        &self.inner
    }

    #[doc(hidden)]
    pub fn inner_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    #[doc(hidden)]
    pub fn inner_pinned_mut(self: Pin<&mut Self>) -> Pin<&mut S> {
        unsafe { self.map_unchecked_mut(|this| &mut this.inner) }
    }
}

impl<Dyn: private::DynTrait + ?Sized, S: Storage> Drop for DynStorage<Dyn, S> {
    fn drop(&mut self) {
        if let Some(drop_inner) = private::StorageVTable::drop_in_place(self.vtable) {
            // SAFETY: the storage data is no longer accessed after the call,
            // and is matched by the vtable as per function contract.
            unsafe { drop_inner(self.inner.ptr_mut()) };
        }
        let layout = private::StorageVTable::layout(self.vtable);
        // SAFETY: the storage data is no longer accessed after the call,
        // and is matched by the vtable as per function contract.
        unsafe { self.inner.drop_in_place(layout) };
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl<Dyn: private::DynTrait<VTable: fmt::Debug> + ?Sized, S: Storage + fmt::Debug> fmt::Debug
    for DynStorage<Dyn, S>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynStorage")
            .field("inner", &self.inner)
            .field("vtable", &self.vtable)
            .finish()
    }
}

pub enum TrySync<F: Future> {
    Sync(F::Output),
    Async(F),
    SyncPolled,
}

impl<F: Future> TrySync<F> {
    #[cfg_attr(coverage_nightly, coverage(off))] // Because of `unreachable_unchecked` branch
    #[inline(always)]
    unsafe fn take_sync(&mut self) -> F::Output {
        match mem::replace(self, TrySync::SyncPolled) {
            TrySync::Sync(res) => res,
            _ => unsafe { hint::unreachable_unchecked() },
        }
    }
}

impl<F: Future> Future for TrySync<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match unsafe { self.get_unchecked_mut() } {
            res @ TrySync::Sync(_) => Poll::Ready(unsafe { res.take_sync() }),
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
