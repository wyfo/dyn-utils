//! The storages backing [`DynObject`](crate::DynObject).

#[cfg(any(feature = "alloc", doc))]
use alloc::boxed::Box as StdBox;
use core::{
    alloc::Layout,
    cell::UnsafeCell,
    hint::unreachable_unchecked,
    marker::{PhantomData, PhantomPinned},
    mem::MaybeUninit,
    pin::Pin,
    ptr::NonNull,
};

pub use elain::{Align, Alignment};

/// Default storage for [`DynObject`](crate::DynObject), and used in [`dyn_trait`](crate::dyn_trait) macro.
pub type DefaultStorage = RawOrBox<{ 128 * size_of::<usize>() }>;

/// A storage that can be used to store dynamic type-erased objects.
///
/// # Safety
///
/// `ptr`/`ptr_mut`/`as_ref`/`as_mut`/`as_pinned_mut` must return a pointer/reference
/// to stored data.
pub unsafe trait Storage: Sized {
    /// Constructs a new storage storing `T`.
    fn new<T>(data: T) -> Self;
    /// Returns a const pointer to stored data.
    fn ptr(&self) -> NonNull<()>;
    /// Returns a mutable pointer to stored data.
    fn ptr_mut(&mut self) -> NonNull<()>;
    /// Returns a reference to stored data.
    ///
    /// # Safety
    ///
    /// Storage must have been constructed with `T`
    unsafe fn as_ref<T>(&self) -> &T {
        // SAFETY: `Self::ptr` returns a const pointer to stored data
        unsafe { self.ptr().cast().as_ref() }
    }
    /// Returns a mutable reference to stored data.
    ///
    /// # Safety
    ///
    /// Storage must have been constructed with `T`
    unsafe fn as_mut<T>(&mut self) -> &mut T {
        // SAFETY: `Self::ptr` returns a mutable pointer to stored data
        unsafe { self.ptr_mut().cast().as_mut() }
    }
    /// Returns a pinned mutable reference to stored data.
    ///
    /// # Safety
    ///
    /// Storage must have been constructed with from `T`
    unsafe fn as_pinned_mut<T>(self: Pin<&mut Self>) -> Pin<&mut T> {
        // SAFETY: data is not moved, and `Self::as_mut` as the same precondition
        unsafe { self.map_unchecked_mut(|this| this.as_mut()) }
    }
    /// Drop the storage in place with the layout of the stored data.
    ///
    /// Stored data should have been dropped in place before calling this method.
    ///
    /// # Safety
    ///
    /// `drop_in_place` must be called once, and the storage must not be used
    /// after. `layout` must be the layout of the data stored.
    unsafe fn drop_in_place(&mut self, layout: Layout);
}

/// A storage that can be constructed from boxed data.
#[cfg(feature = "alloc")]
pub trait FromBox: Storage {
    /// Constructs a new storage storing `T`.
    ///
    /// Data may be moved out the box if it fits in the storage.
    fn from_box<T>(boxed: StdBox<T>) -> Self;
}

/// A raw storage, where data is stored in place.
///
/// Data size and alignment must fit, e.g. be lesser or equal to the generic parameters.
/// This condition is enforced by a constant assertion, which triggers at build time
/// — **it is not triggered by `cargo check`**.
#[derive(Debug)]
#[repr(C)]
pub struct Raw<const SIZE: usize, const ALIGN: usize = { align_of::<usize>() }>
where
    Align<ALIGN>: Alignment,
{
    data: UnsafeCell<MaybeUninit<[u8; SIZE]>>,
    _align: Align<ALIGN>,
    _not_send_sync: PhantomData<*mut ()>,
    _pinned: PhantomPinned,
}

impl<const SIZE: usize, const ALIGN: usize> Raw<SIZE, ALIGN>
where
    Align<ALIGN>: Alignment,
{
    /// Returns `true` if `T` can be stored in the storage.
    pub const fn can_store<T>() -> bool {
        size_of::<T>() <= SIZE && align_of::<T>() <= ALIGN
    }

    /// Constructs a new `Raw` storage, with compile-time assertion that `T` can be stored.
    pub const fn new<T>(data: T) -> Self {
        #[cfg(feature = "const_panic")]
        const {
            let (size, align) = (size_of::<T>(), align_of::<T>());
            #[rustfmt::skip]
            const_panic::concat_assert!(
                Self::can_store::<T>(),
                "object (size=", size, ", align=", align, ")",
                " doesn't fit into Raw<", SIZE, ", ", ALIGN, "> storage"
            );
        }
        #[cfg(not(feature = "const_panic"))]
        const {
            assert!(Self::can_store::<T>());
        }
        // SAFETY: assertion above ensures function contract
        unsafe { Self::new_unchecked::<T>(data) }
    }

    /// # Safety
    ///
    /// `data` must have size and alignment lesser or equal to the generic parameters.
    const unsafe fn new_unchecked<T>(data: T) -> Self {
        let mut raw = Self {
            data: UnsafeCell::new(MaybeUninit::uninit()),
            _align: Align::NEW,
            _not_send_sync: PhantomData,
            _pinned: PhantomPinned,
        };
        // SAFETY: function contract guarantees that `raw.data` size and alignment
        // matches `data` ones; alignment is obtained through `_align` field and `repr(C)`
        unsafe { raw.data.get_mut().as_mut_ptr().cast::<T>().write(data) };
        raw
    }
}

// SAFETY: `ptr`/`ptr_mut` return a pointer to the stored data.
unsafe impl<const SIZE: usize, const ALIGN: usize> Storage for Raw<SIZE, ALIGN>
where
    Align<ALIGN>: Alignment,
{
    fn new<T>(data: T) -> Self {
        Self::new(data)
    }
    fn ptr(&self) -> NonNull<()> {
        NonNull::new(self.data.get()).unwrap().cast()
    }
    fn ptr_mut(&mut self) -> NonNull<()> {
        NonNull::from(self.data.get_mut()).cast()
    }
    unsafe fn drop_in_place(&mut self, _layout: Layout) {}
}

/// A type-erased [`Box`](StdBox).
#[cfg(any(feature = "alloc", doc))]
#[derive(Debug)]
pub struct Box(NonNull<()>);

#[cfg(feature = "alloc")]
impl FromBox for Box {
    fn from_box<T>(data: StdBox<T>) -> Self {
        Self(NonNull::new(StdBox::into_raw(data).cast()).unwrap())
    }
}

// SAFETY: `ptr`/`ptr_mut` return a pointer to the stored data.
#[cfg(feature = "alloc")]
unsafe impl Storage for Box {
    fn new<T>(data: T) -> Self {
        Self::from_box(StdBox::new(data))
    }
    fn ptr(&self) -> NonNull<()> {
        self.0
    }
    fn ptr_mut(&mut self) -> NonNull<()> {
        self.0
    }
    unsafe fn drop_in_place(&mut self, layout: Layout) {
        if layout.size() != 0 {
            // SAFETY: storage has been initialized with `Box<T>`,
            // and `layout` must be `Layout::new::<T>()` as per function contract
            unsafe { alloc::alloc::dealloc(self.0.as_ptr().cast(), layout) };
        }
    }
}

#[derive(Debug)]
enum RawOrBoxInner<const SIZE: usize, const ALIGN: usize = { align_of::<usize>() }>
where
    Align<ALIGN>: Alignment,
{
    Raw(Raw<SIZE, ALIGN>),
    #[cfg(feature = "alloc")]
    Box(Box),
}

/// A [`Raw`] storage with `Box` backup if the object doesn't fit in.
///
/// When `alloc` feature is not enabled, it behaves like [`Raw`].
#[derive(Debug)]
pub struct RawOrBox<const SIZE: usize, const ALIGN: usize = { align_of::<usize>() }>(
    RawOrBoxInner<SIZE, ALIGN>,
)
where
    Align<ALIGN>: Alignment;

#[cfg_attr(coverage_nightly, coverage(off))]
impl<const SIZE: usize, const ALIGN: usize> RawOrBox<SIZE, ALIGN>
where
    Align<ALIGN>: Alignment,
{
    /// Constructs a [`Raw`] variant of `RawOrBox`.
    pub const fn new_raw<T>(data: T) -> Self {
        Self(RawOrBoxInner::Raw(Raw::new(data)))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(feature = "alloc")]
impl<const SIZE: usize, const ALIGN: usize> FromBox for RawOrBox<SIZE, ALIGN>
where
    Align<ALIGN>: Alignment,
{
    fn from_box<T>(data: StdBox<T>) -> Self {
        if Raw::<SIZE, ALIGN>::can_store::<T>() {
            Self::new(*data)
        } else {
            Self(RawOrBoxInner::Box(Box::from_box(data)))
        }
    }
}

// SAFETY: The impl delegates to `Raw`/`Box` which implements `Storage`
// This enum is generic and the variant is chosen according constant predicate,
// so it's not possible to cover all variant for a specific monomorphization.
// https://github.com/taiki-e/cargo-llvm-cov/issues/394
#[cfg_attr(coverage_nightly, coverage(off))]
unsafe impl<const SIZE: usize, const ALIGN: usize> Storage for RawOrBox<SIZE, ALIGN>
where
    Align<ALIGN>: Alignment,
{
    fn new<T>(data: T) -> Self {
        #[cfg(feature = "alloc")]
        if Raw::<SIZE, ALIGN>::can_store::<T>() {
            // SAFETY: size and alignment are checked above
            Self(RawOrBoxInner::Raw(unsafe { Raw::new_unchecked(data) }))
        } else {
            Self(RawOrBoxInner::Box(Box::new(data)))
        }
        #[cfg(not(feature = "alloc"))]
        {
            Self(RawOrBoxInner::Raw(Raw::new(data)))
        }
    }
    fn ptr(&self) -> NonNull<()> {
        match &self.0 {
            RawOrBoxInner::Raw(s) => s.ptr(),
            #[cfg(feature = "alloc")]
            RawOrBoxInner::Box(s) => s.ptr(),
        }
    }
    fn ptr_mut(&mut self) -> NonNull<()> {
        match &mut self.0 {
            RawOrBoxInner::Raw(s) => s.ptr_mut(),
            #[cfg(feature = "alloc")]
            RawOrBoxInner::Box(s) => s.ptr_mut(),
        }
    }
    unsafe fn as_ref<T>(&self) -> &T {
        match &self.0 {
            // SAFETY: same precondition
            RawOrBoxInner::Raw(s) if Raw::<SIZE, ALIGN>::can_store::<T>() => unsafe { s.as_ref() },
            #[cfg(feature = "alloc")]
            // SAFETY: same precondition
            RawOrBoxInner::Box(s) if !Raw::<SIZE, ALIGN>::can_store::<T>() => unsafe { s.as_ref() },
            // SAFETY: storage will always be Raw if it can store `T`
            _ => unsafe { unreachable_unchecked() },
        }
    }
    unsafe fn as_mut<T>(&mut self) -> &mut T {
        match &mut self.0 {
            // SAFETY: same precondition
            RawOrBoxInner::Raw(s) if Raw::<SIZE, ALIGN>::can_store::<T>() => unsafe { s.as_mut() },
            #[cfg(feature = "alloc")]
            // SAFETY: same precondition
            RawOrBoxInner::Box(s) if !Raw::<SIZE, ALIGN>::can_store::<T>() => unsafe { s.as_mut() },
            // SAFETY: storage will always be Raw if it can store `T`
            _ => unsafe { unreachable_unchecked() },
        }
    }
    unsafe fn drop_in_place(&mut self, layout: Layout) {
        match &mut self.0 {
            // SAFETY: same precondition
            RawOrBoxInner::Raw(s) => unsafe { s.drop_in_place(layout) },
            #[cfg(feature = "alloc")]
            // SAFETY: same precondition
            RawOrBoxInner::Box(s) => unsafe { s.drop_in_place(layout) },
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(test)]
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use core::{any::Any, cell::RefCell, mem};

    use elain::{Align, Alignment};

    use crate::{DynObject, storage::Storage};

    trait Test {}
    const _: () = {
        #[derive(Debug)]
        pub struct __Vtable {
            __drop_in_place: Option<unsafe fn(::core::ptr::NonNull<()>)>,
            __layout: ::core::alloc::Layout,
        }
        impl<'__lt> crate::object::DynTrait for dyn Test + '__lt {
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
        unsafe impl<'__lt, __Dyn: Test + '__lt> crate::object::Vtable<__Dyn> for dyn Test + '__lt {
            fn vtable<__Storage: crate::storage::Storage>() -> &'static Self::Vtable {
                &const {
                    __Vtable {
                        __drop_in_place: <Self as crate::object::Vtable<__Dyn>>::DROP_IN_PLACE_FN,
                        __layout: core::alloc::Layout::new::<__Dyn>(),
                    }
                }
            }
        }
    };
    impl Test for () {}
    impl<const N: usize> Test for [u8; N] {}
    impl Test for u64 {}
    type TestObject<'__dyn, S> = DynObject<dyn Test + '__dyn, S>;

    #[test]
    fn raw_alignment() {
        fn check_alignment<const ALIGN: usize>()
        where
            Align<ALIGN>: Alignment,
        {
            let storages = [(); 2].map(TestObject::<super::Raw<0, ALIGN>>::new);
            for s in &storages {
                assert!(s.storage().ptr().cast::<Align<ALIGN>>().is_aligned());
            }
            const { assert!(ALIGN < 2048) };
            assert!(
                storages
                    .iter()
                    .any(|s| !s.storage().ptr().cast::<Align<2048>>().is_aligned())
            );
        }
        check_alignment::<1>();
        check_alignment::<8>();
        check_alignment::<64>();
        check_alignment::<1024>();
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn raw_or_box() {
        fn check_variant<const N: usize>(variant: impl Fn(&super::RawOrBox<8>) -> bool) {
            let array = core::array::from_fn::<u8, N, _>(|i| i as u8);
            let storage = TestObject::<super::RawOrBox<8>>::new(array);
            assert!(variant(storage.storage()));
            assert_eq!(
                unsafe { storage.storage().ptr().cast::<[u8; N]>().read() },
                array
            );
        }
        check_variant::<4>(|s| matches!(s.0, super::RawOrBoxInner::Raw(_)));
        check_variant::<64>(|s| matches!(s.0, super::RawOrBoxInner::Box(_)));

        let storage = TestObject::<super::RawOrBox<8, 1>>::new(0u64);
        assert!(matches!(storage.storage().0, super::RawOrBoxInner::Box(_)));
    }

    struct SetDropped<'a>(&'a mut bool);
    impl Test for SetDropped<'_> {}
    impl Drop for SetDropped<'_> {
        fn drop(&mut self) {
            assert!(!mem::replace(self.0, true));
        }
    }

    #[test]
    fn storage_drop() {
        fn check_drop<S: Storage>() {
            let mut dropped = false;
            let storage = TestObject::<S>::new(SetDropped(&mut dropped));
            assert!(!*unsafe { storage.storage().ptr().cast::<SetDropped>().as_ref() }.0);
            drop(storage);
            assert!(dropped);
        }
        check_drop::<super::Raw<{ size_of::<SetDropped>() }, { align_of::<SetDropped>() }>>();
        #[cfg(feature = "alloc")]
        check_drop::<super::Box>();
        #[cfg(feature = "alloc")]
        check_drop::<super::RawOrBox<{ size_of::<SetDropped>() }>>();
        #[cfg(feature = "alloc")]
        check_drop::<super::RawOrBox<0>>();
    }

    #[test]
    fn storage_dst() {
        fn check_dst<S: Storage>() {
            drop(TestObject::<S>::new(()));
        }
        check_dst::<super::Raw<{ size_of::<SetDropped>() }, { align_of::<SetDropped>() }>>();
        #[cfg(feature = "alloc")]
        check_dst::<super::Box>();
        #[cfg(feature = "alloc")]
        check_dst::<super::RawOrBox<{ size_of::<SetDropped>() }>>();
        #[cfg(feature = "alloc")]
        check_dst::<super::RawOrBox<0>>();
    }

    #[test]
    fn storage_interior_mutability() {
        fn check<S: Storage + core::fmt::Debug>() {
            let obj = DynObject::<dyn Any, S>::new(RefCell::new(false));
            *obj.downcast_ref::<RefCell<bool>>().unwrap().borrow_mut() = true;
            assert!(obj.downcast::<RefCell<bool>>().unwrap().into_inner());
        }
        check::<super::Raw<16>>();
        #[cfg(feature = "alloc")]
        check::<super::Box>();
        check::<super::RawOrBox<16>>();
        #[cfg(feature = "alloc")]
        check::<super::RawOrBox<0>>();
    }
}
