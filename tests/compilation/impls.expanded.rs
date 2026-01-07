const _: () = {
    pub struct __Vtable {
        __drop_in_place: Option<unsafe fn(::core::ptr::NonNull<()>)>,
        __layout: ::core::alloc::Layout,
        poll: unsafe fn(),
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for __Vtable {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field3_finish(
                f,
                "__Vtable",
                "__drop_in_place",
                &self.__drop_in_place,
                "__layout",
                &self.__layout,
                "poll",
                &&self.poll,
            )
        }
    }
    impl<'__lt, __TypeOutput> crate::object::DynTrait
    for dyn Future<Output = __TypeOutput> + '__lt {
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
    unsafe impl<
        '__lt,
        __TypeOutput,
        __Dyn: Future<Output = __TypeOutput> + '__lt,
    > crate::object::Vtable<__Dyn> for dyn Future<Output = __TypeOutput> + '__lt {
        fn vtable<__Storage: crate::storage::Storage>() -> &'static Self::Vtable {
            &const {
                __Vtable {
                    __drop_in_place: <Self as crate::object::Vtable<
                        __Dyn,
                    >>::DROP_IN_PLACE_FN,
                    __layout: core::alloc::Layout::new::<__Dyn>(),
                    #[allow(
                        clippy::missing_transmute_annotations,
                        clippy::useless_transmute
                    )]
                    poll: unsafe {
                        ::core::mem::transmute::<
                            fn(
                                ::core::pin::Pin<&mut __Storage>,
                                &mut core::task::Context<'_>,
                            ) -> core::task::Poll<__Dyn::Output>,
                            unsafe fn(),
                        >(|__self, cx| ::core::mem::transmute(
                            __Dyn::poll(
                                __self.as_pinned_mut(),
                                ::core::mem::transmute(cx),
                            ),
                        ))
                    },
                }
            }
        }
    }
    impl<'__lt, __TypeOutput, __Storage: crate::storage::Storage> Future
    for crate::DynObject<dyn Future<Output = __TypeOutput> + '__lt, __Storage> {
        type Output = __TypeOutput;
        fn poll(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Self::Output> {
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
    pub struct __Vtable {
        __drop_in_place: Option<unsafe fn(::core::ptr::NonNull<()>)>,
        __layout: ::core::alloc::Layout,
        poll: unsafe fn(),
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for __Vtable {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field3_finish(
                f,
                "__Vtable",
                "__drop_in_place",
                &self.__drop_in_place,
                "__layout",
                &self.__layout,
                "poll",
                &&self.poll,
            )
        }
    }
    impl<'__lt, __TypeOutput> crate::object::DynTrait
    for dyn Future<Output = __TypeOutput> + '__lt + Send {
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
    unsafe impl<
        '__lt,
        __TypeOutput,
        __Dyn: Future<Output = __TypeOutput> + '__lt + Send,
    > crate::object::Vtable<__Dyn> for dyn Future<Output = __TypeOutput> + '__lt + Send {
        fn vtable<__Storage: crate::storage::Storage>() -> &'static Self::Vtable {
            &const {
                __Vtable {
                    __drop_in_place: <Self as crate::object::Vtable<
                        __Dyn,
                    >>::DROP_IN_PLACE_FN,
                    __layout: core::alloc::Layout::new::<__Dyn>(),
                    #[allow(
                        clippy::missing_transmute_annotations,
                        clippy::useless_transmute
                    )]
                    poll: unsafe {
                        ::core::mem::transmute::<
                            fn(
                                ::core::pin::Pin<&mut __Storage>,
                                &mut core::task::Context<'_>,
                            ) -> core::task::Poll<__Dyn::Output>,
                            unsafe fn(),
                        >(|__self, cx| ::core::mem::transmute(
                            __Dyn::poll(
                                __self.as_pinned_mut(),
                                ::core::mem::transmute(cx),
                            ),
                        ))
                    },
                }
            }
        }
    }
    impl<'__lt, __TypeOutput, __Storage: crate::storage::Storage> Future
    for crate::DynObject<dyn Future<Output = __TypeOutput> + '__lt + Send, __Storage> {
        type Output = __TypeOutput;
        fn poll(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Self::Output> {
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
    pub struct __Vtable {
        __drop_in_place: Option<unsafe fn(::core::ptr::NonNull<()>)>,
        __layout: ::core::alloc::Layout,
        next: unsafe fn(),
        size_hint: unsafe fn(),
        nth: unsafe fn(),
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for __Vtable {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field5_finish(
                f,
                "__Vtable",
                "__drop_in_place",
                &self.__drop_in_place,
                "__layout",
                &self.__layout,
                "next",
                &self.next,
                "size_hint",
                &self.size_hint,
                "nth",
                &&self.nth,
            )
        }
    }
    impl<'__lt, __TypeItem> crate::object::DynTrait
    for dyn Iterator<Item = __TypeItem> + '__lt {
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
    unsafe impl<
        '__lt,
        __TypeItem,
        __Dyn: Iterator<Item = __TypeItem> + '__lt,
    > crate::object::Vtable<__Dyn> for dyn Iterator<Item = __TypeItem> + '__lt {
        fn vtable<__Storage: crate::storage::Storage>() -> &'static Self::Vtable {
            &const {
                __Vtable {
                    __drop_in_place: <Self as crate::object::Vtable<
                        __Dyn,
                    >>::DROP_IN_PLACE_FN,
                    __layout: core::alloc::Layout::new::<__Dyn>(),
                    #[allow(
                        clippy::missing_transmute_annotations,
                        clippy::useless_transmute
                    )]
                    next: unsafe {
                        ::core::mem::transmute::<
                            fn(&mut __Storage) -> Option<__Dyn::Item>,
                            unsafe fn(),
                        >(|__self| ::core::mem::transmute(__Dyn::next(__self.as_mut())))
                    },
                    #[allow(
                        clippy::missing_transmute_annotations,
                        clippy::useless_transmute
                    )]
                    size_hint: unsafe {
                        ::core::mem::transmute::<
                            fn(&__Storage) -> (usize, Option<usize>),
                            unsafe fn(),
                        >(|__self| ::core::mem::transmute(
                            __Dyn::size_hint(__self.as_ref()),
                        ))
                    },
                    #[allow(
                        clippy::missing_transmute_annotations,
                        clippy::useless_transmute
                    )]
                    nth: unsafe {
                        ::core::mem::transmute::<
                            fn(&mut __Storage, usize) -> Option<__Dyn::Item>,
                            unsafe fn(),
                        >(|__self, n| ::core::mem::transmute(
                            __Dyn::nth(__self.as_mut(), ::core::mem::transmute(n)),
                        ))
                    },
                }
            }
        }
    }
    impl<'__lt, __TypeItem, __Storage: crate::storage::Storage> Iterator
    for crate::DynObject<dyn Iterator<Item = __TypeItem> + '__lt, __Storage> {
        type Item = __TypeItem;
        fn next(&mut self) -> Option<Self::Item> {
            unsafe {
                ::core::mem::transmute::<
                    unsafe fn(),
                    fn(&mut __Storage) -> Option<Self::Item>,
                >(self.vtable().next)(self.storage_mut())
            }
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            unsafe {
                ::core::mem::transmute::<
                    unsafe fn(),
                    fn(&__Storage) -> (usize, Option<usize>),
                >(self.vtable().size_hint)(self.storage())
            }
        }
        fn nth(&mut self, n: usize) -> Option<Self::Item> {
            unsafe {
                ::core::mem::transmute::<
                    unsafe fn(),
                    fn(&mut __Storage, usize) -> Option<Self::Item>,
                >(self.vtable().nth)(self.storage_mut(), n)
            }
        }
    }
};
