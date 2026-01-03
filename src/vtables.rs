use core::{
    any::{Any, TypeId},
    mem,
    mem::ManuallyDrop,
    pin::Pin,
    task::{Context, Poll},
};

use crate::{
    DynStorage,
    private::{DynTrait, DynVTable, NewVTable, StorageVTable},
    storage::Storage,
};

#[derive(Debug)]
pub struct AnyVTable {
    __dyn_vtable: DynVTable,
    type_id: TypeId,
}

impl StorageVTable for AnyVTable {
    fn dyn_vtable(&self) -> &DynVTable {
        &self.__dyn_vtable
    }
}

impl<'__dyn> DynTrait for dyn Any + '__dyn {
    type VTable = AnyVTable;
}

unsafe impl<'__dyn, __Dyn: Any> NewVTable<__Dyn> for dyn Any + '__dyn {
    fn new_vtable<__Storage: Storage>() -> &'static Self::VTable {
        &const {
            AnyVTable {
                __dyn_vtable: DynVTable::new::<__Dyn>(),
                type_id: TypeId::of::<__Dyn>(),
            }
        }
    }
}

impl<'__dyn, __Storage: Storage> DynStorage<dyn Any + '__dyn, __Storage> {
    pub fn type_id(&self) -> TypeId {
        self.vtable().type_id
    }

    pub fn is<T: Any>(&self) -> bool {
        self.type_id() == TypeId::of::<T>()
    }

    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.is::<T>()
            .then(|| unsafe { self.inner().ptr().cast().as_ref() })
    }

    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.is::<T>()
            .then(|| unsafe { self.inner_mut().ptr_mut().cast().as_mut() })
    }

    // TODO understand why it prevents 100% coverage
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn downcast<T: Any>(self) -> Result<T, Self> {
        if self.is::<T>() {
            Ok(unsafe { ManuallyDrop::new(self).inner_mut().ptr_mut().cast().read() })
        } else {
            Err(self)
        }
    }
}

#[derive(Debug)]
pub struct FutureVTable {
    __dyn_vtable: DynVTable,
    poll: unsafe fn(),
}

impl StorageVTable for FutureVTable {
    fn dyn_vtable(&self) -> &DynVTable {
        &self.__dyn_vtable
    }
}

impl<'__dyn, __TypeOutput> DynTrait for dyn Future<Output = __TypeOutput> + '__dyn {
    type VTable = FutureVTable;
}

impl<'__dyn, __TypeOutput> DynTrait for dyn Future<Output = __TypeOutput> + Send + '__dyn {
    type VTable = FutureVTable;
}

unsafe impl<'__dyn, __Dyn: Future> NewVTable<__Dyn>
    for dyn Future<Output = __Dyn::Output> + '__dyn
{
    fn new_vtable<__Storage: Storage>() -> &'static Self::VTable {
        &const {
            FutureVTable {
                __dyn_vtable: DynVTable::new::<__Dyn>(),
                poll: unsafe {
                    mem::transmute::<
                        unsafe fn(Pin<&mut __Storage>, &mut Context) -> Poll<__Dyn::Output>,
                        unsafe fn(),
                    >(|__self, cx| __self.as_pinned_mut::<__Dyn>().poll(cx))
                },
            }
        }
    }
}

unsafe impl<'__dyn, __Dyn: Future> NewVTable<__Dyn>
    for dyn Future<Output = __Dyn::Output> + Send + '__dyn
{
    fn new_vtable<__Storage: Storage>() -> &'static Self::VTable {
        &const {
            FutureVTable {
                __dyn_vtable: DynVTable::new::<__Dyn>(),
                poll: unsafe {
                    mem::transmute::<
                        unsafe fn(Pin<&mut __Storage>, &mut Context) -> Poll<__Dyn::Output>,
                        unsafe fn(),
                    >(|__self, cx| __self.as_pinned_mut::<__Dyn>().poll(cx))
                },
            }
        }
    }
}

impl<'__dyn, __Storage: Storage, __TypeOutput> Future
    for DynStorage<dyn Future<Output = __TypeOutput> + '__dyn, __Storage>
{
    type Output = __TypeOutput;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            mem::transmute::<
                unsafe fn(),
                unsafe fn(Pin<&mut __Storage>, &mut Context) -> Poll<__TypeOutput>,
            >(self.vtable().poll)(self.inner_pinned_mut(), cx)
        }
    }
}

impl<'__dyn, __Storage: Storage, __TypeOutput> Future
    for DynStorage<dyn Future<Output = __TypeOutput> + Send + '__dyn, __Storage>
{
    type Output = __TypeOutput;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            mem::transmute::<
                unsafe fn(),
                unsafe fn(Pin<&mut __Storage>, &mut Context) -> Poll<__TypeOutput>,
            >(self.vtable().poll)(self.inner_pinned_mut(), cx)
        }
    }
}

#[derive(Debug)]
pub struct IteratorVTable {
    __dyn_vtable: DynVTable,
    next: unsafe fn(),
    size_hint: unsafe fn(),
    nth: unsafe fn(),
}

impl StorageVTable for IteratorVTable {
    fn dyn_vtable(&self) -> &DynVTable {
        &self.__dyn_vtable
    }
}

unsafe impl<'__dyn, __Dyn: Iterator> NewVTable<__Dyn>
    for dyn Iterator<Item = __Dyn::Item> + '__dyn
{
    fn new_vtable<__Storage: Storage>() -> &'static Self::VTable {
        &const {
            IteratorVTable {
                __dyn_vtable: DynVTable::new::<__Dyn>(),
                next: unsafe {
                    mem::transmute::<unsafe fn(&mut __Storage) -> Option<__Dyn::Item>, unsafe fn()>(
                        |__self| __self.as_mut::<__Dyn>().next(),
                    )
                },
                size_hint: unsafe {
                    mem::transmute::<unsafe fn(&__Storage) -> (usize, Option<usize>), unsafe fn()>(
                        |__self| __self.as_ref::<__Dyn>().size_hint(),
                    )
                },
                nth: unsafe {
                    mem::transmute::<
                        unsafe fn(&mut __Storage, usize) -> Option<__Dyn::Item>,
                        unsafe fn(),
                    >(|__self, n| __self.as_mut::<__Dyn>().nth(n))
                },
            }
        }
    }
}

impl<'__dyn, __TypeItem> DynTrait for dyn Iterator<Item = __TypeItem> + '__dyn {
    type VTable = IteratorVTable;
}

impl<'__dyn, __TypeItem, __Storage: Storage> Iterator
    for DynStorage<dyn Iterator<Item = __TypeItem> + '__dyn, __Storage>
{
    type Item = __TypeItem;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            mem::transmute::<unsafe fn(), unsafe fn(&mut __Storage) -> Option<__TypeItem>>(
                self.vtable().next,
            )(self.inner_mut())
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        unsafe {
            mem::transmute::<unsafe fn(), unsafe fn(&__Storage) -> (usize, Option<usize>)>(
                self.vtable().size_hint,
            )(self.inner())
        }
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        unsafe {
            mem::transmute::<unsafe fn(), unsafe fn(&mut __Storage, usize) -> Option<__TypeItem>>(
                self.vtable().nth,
            )(self.inner_mut(), n)
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(test)]
mod tests {
    use core::any::Any;

    use futures::FutureExt;

    use crate::DynStorage;

    fn assert_send<T: Send>(_: &T) {}

    #[test]
    fn dyn_any() {
        let mut any = DynStorage::<dyn Any>::new(false);
        assert_eq!(any.downcast_ref::<()>(), None);
        assert_eq!(any.downcast_ref::<bool>(), Some(&false));
        assert_eq!(any.downcast_mut::<()>(), None);
        *any.downcast_mut::<bool>().unwrap() = true;
        let storage = any.downcast::<()>().unwrap_err();
        assert!(storage.downcast::<bool>().unwrap());
        drop(DynStorage::<dyn Any>::new(()));
    }

    #[test]
    fn dyn_future() {
        let n = 42;
        let future = DynStorage::<dyn Future<Output = usize>>::new(async { n });
        assert_eq!(future.now_or_never(), Some(42));
    }

    #[test]
    fn dyn_future_send() {
        let n = 42;
        let future = DynStorage::<dyn Future<Output = usize> + Send>::new(async { n });
        assert_send(&future);
        assert_eq!(future.now_or_never(), Some(42));
    }

    #[test]
    fn dyn_iterator() {
        let mut iter = DynStorage::<dyn Iterator<Item = usize>>::new([0, 1, 2, 3].into_iter());
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert_eq!(iter.nth(2), Some(2));
        assert_eq!(iter.next(), Some(3));
    }
}
