use core::{
    alloc::Layout,
    hint, mem,
    pin::Pin,
    ptr::NonNull,
    task::{Context, Poll},
};

use crate::storage::Storage;

pub trait DynTrait {
    type VTable: 'static;
    fn drop_in_place(vtable: &Self::VTable) -> Option<unsafe fn(NonNull<()>)>;
    fn layout(vtable: &Self::VTable) -> Layout;
}

/// # Safety
///
/// - `DynTrait::layout` must return `core::alloc::Layout::new::<T>()`.
/// - `DynTrait::drop_in_place` must returns `crate::private::drop_in_place::<T>()`
pub unsafe trait VTable<T>: DynTrait {
    fn vtable<S: Storage>() -> &'static Self::VTable;
}

pub const fn drop_in_place_fn<T>() -> Option<unsafe fn(NonNull<()>)> {
    if mem::needs_drop::<T>() {
        Some(|ptr_mut| unsafe { ptr_mut.cast::<T>().drop_in_place() })
    } else {
        None
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

    use crate::private::TrySync;

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
