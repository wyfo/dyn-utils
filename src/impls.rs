use core::{
    pin::Pin,
    task::{Context, Poll},
};

// `dyn_object` cannot be used because `Any` has a blanket impl
// anyway, it allows optimizing type_id as a field and not as a method
macro_rules! any_impl {
    ($dyn_any:ty) => {
        const _: () = {
            #[derive(Debug)]
            pub struct __VTable {
                __drop_in_place: Option<unsafe fn(core::ptr::NonNull<()>)>,
                __layout: core::alloc::Layout,
                type_id: core::any::TypeId,
            }

            impl crate::private::DynTrait for $dyn_any {
                type VTable = __VTable;
                fn drop_in_place(
                    vtable: &Self::VTable,
                ) -> Option<unsafe fn(core::ptr::NonNull<()>)> {
                    vtable.__drop_in_place
                }
                fn layout(vtable: &Self::VTable) -> core::alloc::Layout {
                    vtable.__layout
                }
            }

            // SAFETY: vtable fields respect trait contract
            unsafe impl<__Dyn: Any> crate::private::VTable<__Dyn> for $dyn_any {
                fn vtable<__Storage: crate::storage::Storage>() -> &'static Self::VTable {
                    &const {
                        __VTable {
                            __drop_in_place: crate::private::drop_in_place_fn::<__Dyn>(),
                            __layout: core::alloc::Layout::new::<__Dyn>(),
                            type_id: core::any::TypeId::of::<__Dyn>(),
                        }
                    }
                }
            }
        };

        impl<__Storage: crate::storage::Storage> crate::DynObject<$dyn_any, __Storage> {
            /// Returns the [`TypeId`](core::any::TypeId) of the underlying concrete type.
            pub fn type_id(&self) -> core::any::TypeId {
                self.vtable().type_id
            }

            /// Returns `true` if the inner type is the same as `T`.
            pub fn is<T: Any>(&self) -> bool {
                self.type_id() == TypeId::of::<T>()
            }

            /// Returns some reference to the inner value if it is of type `T`,
            /// or `None` if it isn’t.
            pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
                // SAFETY: `is` ensures that the storage has been initialized with `T`
                self.is::<T>().then(|| unsafe { self.storage().as_ref() })
            }

            /// Returns some mutable reference to the inner value if it is of type `T`,
            /// or `None` if it isn’t.
            pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
                self.is::<T>()
                    // SAFETY: `is` ensures that the storage has been initialized with `T`
                    .then(|| unsafe { self.storage_mut().as_mut() })
            }

            /// Attempts to downcast the object to a concrete type.
            // TODO understand why it prevents 100% coverage
            #[cfg_attr(coverage_nightly, coverage(off))]
            pub fn downcast<T: Any>(self) -> Result<T, Self> {
                if self.is::<T>() {
                    let mut this = core::mem::ManuallyDrop::new(self);
                    let storage = this.storage_mut();
                    // SAFETY: `is` ensures that the storage has been initialized with `T`
                    let obj = unsafe { storage.ptr_mut().cast().read() };
                    // SAFETY: the storage is no longer used after,
                    // and `is` ensures that the storage has been initialized with `T`
                    unsafe { __Storage::drop_in_place(storage, core::alloc::Layout::new::<T>()) };
                    Ok(obj)
                } else {
                    Err(self)
                }
            }
        }
    };
}
pub(crate) use any_impl;

#[cfg(test)]
macro_rules! any_test {
    ($test:ident, $dyn_any:ty) => {
        #[test]
        fn $test() {
            struct Droppable;
            impl Drop for Droppable {
                fn drop(&mut self) {}
            }
            let mut any = crate::DynObject::<dyn Any>::new(false);
            assert_eq!(any.downcast_ref::<()>(), None);
            assert_eq!(any.downcast_ref::<bool>(), Some(&false));
            assert_eq!(any.downcast_mut::<()>(), None);
            *any.downcast_mut::<bool>().unwrap() = true;
            let storage = any.downcast::<()>().unwrap_err();
            assert!(storage.downcast::<bool>().unwrap());
            drop(crate::DynObject::<dyn Any>::new(()));
            drop(crate::DynObject::<dyn Any>::new(Droppable));
            #[cfg(feature = "alloc")]
            let _ = crate::DynObject::<dyn Any, crate::storage::Box>::new(false).downcast::<bool>();
        }
    };
}
#[cfg(test)]
pub(crate) use any_test;

#[crate::dyn_object(crate = crate, remote = Future)]
#[crate::dyn_object(crate = crate, remote = Future, bounds = Send)]
trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

#[crate::dyn_object(crate = crate, remote = Iterator)]
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
    fn size_hint(&self) -> (usize, Option<usize>);
    fn nth(&mut self, n: usize) -> Option<Self::Item>;
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(test)]
mod tests {
    use futures::FutureExt;

    use crate::DynObject;

    fn assert_send<T: Send>(_: &T) {}

    #[test]
    fn dyn_future() {
        let n = 42;
        let future = DynObject::<dyn Future<Output = usize>>::new(async { n });
        assert_eq!(future.now_or_never(), Some(42));
    }

    #[test]
    fn dyn_future_send() {
        let n = 42;
        let future = DynObject::<dyn Future<Output = usize> + Send>::new(async { n });
        assert_send(&future);
        assert_eq!(future.now_or_never(), Some(42));
    }

    #[test]
    fn dyn_iterator() {
        let mut iter = DynObject::<dyn Iterator<Item = usize>>::new([0, 1, 2, 3].into_iter());
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert_eq!(iter.nth(2), Some(2));
        assert_eq!(iter.next(), Some(3));
    }
}
