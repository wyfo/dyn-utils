// `dyn_object` cannot be used because `Any` has a blanket impl
// anyway, it allows optimizing type_id as a field and not as a method
macro_rules! any_impl {
    ($dyn_any:ty) => {
        const _: () = {
            #[derive(Debug)]
            pub struct __Vtable {
                __drop_in_place: Option<unsafe fn(core::ptr::NonNull<()>)>,
                __layout: core::alloc::Layout,
                type_id: core::any::TypeId,
            }

            impl crate::object::DynTrait for $dyn_any {
                type Vtable = __Vtable;
                fn drop_in_place_fn(
                    vtable: &Self::Vtable,
                ) -> Option<unsafe fn(core::ptr::NonNull<()>)> {
                    vtable.__drop_in_place
                }
                fn layout(vtable: &Self::Vtable) -> core::alloc::Layout {
                    vtable.__layout
                }
            }

            // SAFETY: vtable fields respect trait contract
            unsafe impl<__Dyn: core::any::Any> crate::object::Vtable<__Dyn> for $dyn_any {
                fn vtable<__Storage: crate::storage::Storage>() -> &'static Self::Vtable {
                    &const {
                        __Vtable {
                            __drop_in_place:
                                <Self as crate::object::Vtable<__Dyn>>::DROP_IN_PLACE_FN,
                            __layout: core::alloc::Layout::new::<__Dyn>(),
                            type_id: core::any::TypeId::of::<__Dyn>(),
                        }
                    }
                }
            }

            impl<__Storage: crate::storage::Storage> crate::DynObject<$dyn_any, __Storage> {
                /// Returns the [`TypeId`](core::any::TypeId) of the underlying concrete type.
                pub fn type_id(&self) -> core::any::TypeId {
                    self.vtable().type_id
                }

                /// Returns `true` if the inner type is the same as `T`.
                pub fn is<T: Any>(&self) -> bool {
                    self.type_id() == core::any::TypeId::of::<T>()
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
                        unsafe {
                            __Storage::drop_in_place(storage, core::alloc::Layout::new::<T>())
                        };
                        Ok(obj)
                    } else {
                        Err(self)
                    }
                }
            }
        };
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

const _: () = {
    #[derive(Debug)]
    pub struct __Vtable {
        __drop_in_place: Option<unsafe fn(::core::ptr::NonNull<()>)>,
        __layout: ::core::alloc::Layout,
        poll: unsafe fn(),
    }
    impl<'__lt, __TypeOutput> crate::object::DynTrait for dyn Future<Output = __TypeOutput> + '__lt {
        type Vtable = __Vtable;
        fn drop_in_place_fn(vtable: &Self::Vtable) -> Option<unsafe fn(core::ptr::NonNull<()>)> {
            vtable.__drop_in_place
        }
        fn layout(vtable: &Self::Vtable) -> core::alloc::Layout {
            vtable.__layout
        }
    }
    // SAFETY: vtable fields respect trait contract
    unsafe impl<'__lt, __TypeOutput, __Dyn: Future<Output = __TypeOutput> + '__lt>
        crate::object::Vtable<__Dyn> for dyn Future<Output = __TypeOutput> + '__lt
    {
        fn vtable<__Storage: crate::storage::Storage>() -> &'static Self::Vtable {
            &const {
                __Vtable {
                    __drop_in_place: <Self as crate::object::Vtable<__Dyn>>::DROP_IN_PLACE_FN,
                    __layout: core::alloc::Layout::new::<__Dyn>(),
                    #[allow(
                        clippy::missing_transmute_annotations,
                        clippy::useless_transmute
                    )]
                    // SAFETY: transmutation are only used to erase lifetime,
                    // the real lifetime being enforced in the trait implementation
                    poll: unsafe {
                        ::core::mem::transmute::<
                            fn(
                                ::core::pin::Pin<&mut __Storage>,
                                &mut core::task::Context<'_>,
                            ) -> core::task::Poll<__Dyn::Output>,
                            unsafe fn(),
                        >(|__self, cx| {
                            ::core::mem::transmute(__Dyn::poll(
                                __self.as_pinned_mut(),
                                ::core::mem::transmute(cx),
                            ))
                        })
                    },
                }
            }
        }
    }
    impl<'__lt, __TypeOutput, __Storage: crate::storage::Storage> Future
        for crate::DynObject<dyn Future<Output = __TypeOutput> + '__lt, __Storage>
    {
        type Output = __TypeOutput;
        fn poll(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Self::Output> {
            // SAFETY: the vtable method has been initialized with the given type
            unsafe {
                ::core::mem::transmute::<
                    unsafe fn(),
                    fn(
                        ::core::pin::Pin<&mut __Storage>,
                        &mut core::task::Context<'_>,
                    ) -> core::task::Poll<Self::Output>,
                >(self.vtable().poll)(self.storage_pinned_mut(), cx)
            }
        }
    }
};
const _: () = {
    #[derive(Debug)]
    pub struct __Vtable {
        __drop_in_place: Option<unsafe fn(::core::ptr::NonNull<()>)>,
        __layout: ::core::alloc::Layout,
        poll: unsafe fn(),
    }
    impl<'__lt, __TypeOutput> crate::object::DynTrait
        for dyn Future<Output = __TypeOutput> + '__lt + Send
    {
        type Vtable = __Vtable;
        fn drop_in_place_fn(vtable: &Self::Vtable) -> Option<unsafe fn(core::ptr::NonNull<()>)> {
            vtable.__drop_in_place
        }
        fn layout(vtable: &Self::Vtable) -> core::alloc::Layout {
            vtable.__layout
        }
    }
    // SAFETY: vtable fields respect trait contract
    unsafe impl<'__lt, __TypeOutput, __Dyn: Future<Output = __TypeOutput> + '__lt + Send>
        crate::object::Vtable<__Dyn> for dyn Future<Output = __TypeOutput> + '__lt + Send
    {
        fn vtable<__Storage: crate::storage::Storage>() -> &'static Self::Vtable {
            &const {
                __Vtable {
                    __drop_in_place: <Self as crate::object::Vtable<__Dyn>>::DROP_IN_PLACE_FN,
                    __layout: core::alloc::Layout::new::<__Dyn>(),
                    #[allow(
                        clippy::missing_transmute_annotations,
                        clippy::useless_transmute
                    )]
                    // SAFETY: transmutation are only used to erase lifetime,
                    // the real lifetime being enforced in the trait implementation
                    poll: unsafe {
                        ::core::mem::transmute::<
                            fn(
                                ::core::pin::Pin<&mut __Storage>,
                                &mut core::task::Context<'_>,
                            ) -> core::task::Poll<__Dyn::Output>,
                            unsafe fn(),
                        >(|__self, cx| {
                            ::core::mem::transmute(__Dyn::poll(
                                __self.as_pinned_mut(),
                                ::core::mem::transmute(cx),
                            ))
                        })
                    },
                }
            }
        }
    }
    impl<'__lt, __TypeOutput, __Storage: crate::storage::Storage> Future
        for crate::DynObject<dyn Future<Output = __TypeOutput> + '__lt + Send, __Storage>
    {
        type Output = __TypeOutput;
        fn poll(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Self::Output> {
            // SAFETY: the vtable method has been initialized with the given type
            unsafe {
                ::core::mem::transmute::<
                    unsafe fn(),
                    fn(
                        ::core::pin::Pin<&mut __Storage>,
                        &mut core::task::Context<'_>,
                    ) -> core::task::Poll<Self::Output>,
                >(self.vtable().poll)(self.storage_pinned_mut(), cx)
            }
        }
    }
};

const _: () = {
    #[derive(Debug)]
    pub struct __Vtable {
        __drop_in_place: Option<unsafe fn(::core::ptr::NonNull<()>)>,
        __layout: ::core::alloc::Layout,
        next: unsafe fn(),
        size_hint: unsafe fn(),
        nth: unsafe fn(),
    }
    impl<'__lt, __TypeItem> crate::object::DynTrait for dyn Iterator<Item = __TypeItem> + '__lt {
        type Vtable = __Vtable;
        fn drop_in_place_fn(vtable: &Self::Vtable) -> Option<unsafe fn(core::ptr::NonNull<()>)> {
            vtable.__drop_in_place
        }
        fn layout(vtable: &Self::Vtable) -> core::alloc::Layout {
            vtable.__layout
        }
    }
    // SAFETY: vtable fields respect trait contract
    unsafe impl<'__lt, __TypeItem, __Dyn: Iterator<Item = __TypeItem> + '__lt>
        crate::object::Vtable<__Dyn> for dyn Iterator<Item = __TypeItem> + '__lt
    {
        fn vtable<__Storage: crate::storage::Storage>() -> &'static Self::Vtable {
            &const {
                __Vtable {
                    __drop_in_place: <Self as crate::object::Vtable<__Dyn>>::DROP_IN_PLACE_FN,
                    __layout: core::alloc::Layout::new::<__Dyn>(),
                    #[allow(
                        clippy::missing_transmute_annotations,
                        clippy::useless_transmute
                    )]
                    // SAFETY: transmutation are only used to erase lifetime,
                    // the real lifetime being enforced in the trait implementation
                    next: unsafe {
                        ::core::mem::transmute::<
                            fn(&mut __Storage) -> Option<__Dyn::Item>,
                            unsafe fn(),
                        >(|__self| {
                            ::core::mem::transmute(__Dyn::next(__self.as_mut()))
                        })
                    },
                    #[allow(
                        clippy::missing_transmute_annotations,
                        clippy::useless_transmute
                    )]
                    // SAFETY: transmutation are only used to erase lifetime,
                    // the real lifetime being enforced in the trait implementation
                    size_hint: unsafe {
                        ::core::mem::transmute::<
                            fn(&__Storage) -> (usize, Option<usize>),
                            unsafe fn(),
                        >(|__self| {
                            ::core::mem::transmute(__Dyn::size_hint(__self.as_ref()))
                        })
                    },
                    #[allow(
                        clippy::missing_transmute_annotations,
                        clippy::useless_transmute
                    )]
                    // SAFETY: transmutation are only used to erase lifetime,
                    // the real lifetime being enforced in the trait implementation
                    nth: unsafe {
                        ::core::mem::transmute::<
                            fn(&mut __Storage, usize) -> Option<__Dyn::Item>,
                            unsafe fn(),
                        >(|__self, n| {
                            ::core::mem::transmute(__Dyn::nth(
                                __self.as_mut(),
                                ::core::mem::transmute(n),
                            ))
                        })
                    },
                }
            }
        }
    }
    impl<'__lt, __TypeItem, __Storage: crate::storage::Storage> Iterator
        for crate::DynObject<dyn Iterator<Item = __TypeItem> + '__lt, __Storage>
    {
        type Item = __TypeItem;
        fn next(&mut self) -> Option<Self::Item> {
            // SAFETY: the vtable method has been initialized with the given type
            unsafe {
                ::core::mem::transmute::<unsafe fn(), fn(&mut __Storage) -> Option<Self::Item>>(
                    self.vtable().next,
                )(self.storage_mut())
            }
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            // SAFETY: the vtable method has been initialized with the given type
            unsafe {
                ::core::mem::transmute::<unsafe fn(), fn(&__Storage) -> (usize, Option<usize>)>(
                    self.vtable().size_hint,
                )(self.storage())
            }
        }
        fn nth(&mut self, n: usize) -> Option<Self::Item> {
            // SAFETY: the vtable method has been initialized with the given type
            unsafe {
                ::core::mem::transmute::<unsafe fn(), fn(&mut __Storage, usize) -> Option<Self::Item>>(
                    self.vtable().nth,
                )(self.storage_mut(), n)
            }
        }
    }
};

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
