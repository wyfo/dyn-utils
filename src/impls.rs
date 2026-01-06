use core::{
    any::{Any, TypeId},
    mem::ManuallyDrop,
    pin::Pin,
    task::{Context, Poll},
};

const _: () = {
    #[derive(Debug)]
    pub struct __VTable {
        __drop_in_place: Option<unsafe fn(core::ptr::NonNull<()>)>,
        __layout: core::alloc::Layout,
        type_id: TypeId,
    }

    impl<'__dyn> crate::private::DynTrait for dyn Any + '__dyn {
        type VTable = __VTable;
        fn drop_in_place(vtable: &Self::VTable) -> Option<unsafe fn(core::ptr::NonNull<()>)> {
            vtable.__drop_in_place
        }
        fn layout(vtable: &Self::VTable) -> core::alloc::Layout {
            vtable.__layout
        }
    }

    unsafe impl<'__dyn, __Dyn: Any> crate::private::VTable<__Dyn> for dyn Any + '__dyn {
        fn vtable<__Storage: crate::storage::Storage>() -> &'static Self::VTable {
            &const {
                __VTable {
                    __drop_in_place: if core::mem::needs_drop::<__Dyn>() {
                        Some(|ptr_mut| unsafe { ptr_mut.cast::<__Dyn>().drop_in_place() })
                    } else {
                        None
                    },
                    __layout: const { core::alloc::Layout::new::<__Dyn>() },
                    type_id: TypeId::of::<__Dyn>(),
                }
            }
        }
    }
};

impl<'__dyn, __Storage: crate::storage::Storage> crate::DynObject<dyn Any + '__dyn, __Storage> {
    pub fn type_id(&self) -> TypeId {
        self.vtable().type_id
    }

    pub fn is<T: Any>(&self) -> bool {
        self.type_id() == TypeId::of::<T>()
    }

    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.is::<T>()
            .then(|| unsafe { self.storage().ptr().cast().as_ref() })
    }

    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.is::<T>()
            .then(|| unsafe { self.storage_mut().ptr_mut().cast().as_mut() })
    }

    // TODO understand why it prevents 100% coverage
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn downcast<T: Any>(self) -> Result<T, Self> {
        if self.is::<T>() {
            Ok(unsafe {
                ManuallyDrop::new(self)
                    .storage_mut()
                    .ptr_mut()
                    .cast()
                    .read()
            })
        } else {
            Err(self)
        }
    }
}

#[crate::dyn_storage(crate = crate, remote = Future)]
#[crate::dyn_storage(crate = crate, remote = Future, bounds = Send)]
trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

#[crate::dyn_storage(crate = crate, remote = Iterator)]
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
    fn size_hint(&self) -> (usize, Option<usize>);
    fn nth(&mut self, n: usize) -> Option<Self::Item>;
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(test)]
mod tests {
    use core::any::Any;

    use futures::FutureExt;

    use crate::DynObject;

    fn assert_send<T: Send>(_: &T) {}

    struct Droppable;
    impl Drop for Droppable {
        fn drop(&mut self) {}
    }

    #[test]
    fn dyn_any() {
        let mut any = DynObject::<dyn Any>::new(false);
        assert_eq!(any.downcast_ref::<()>(), None);
        assert_eq!(any.downcast_ref::<bool>(), Some(&false));
        assert_eq!(any.downcast_mut::<()>(), None);
        *any.downcast_mut::<bool>().unwrap() = true;
        let storage = any.downcast::<()>().unwrap_err();
        assert!(storage.downcast::<bool>().unwrap());
        drop(DynObject::<dyn Any>::new(()));
        drop(DynObject::<dyn Any>::new(Droppable));
    }

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
