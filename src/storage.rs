#[cfg(feature = "alloc")]
use alloc::boxed::Box as StdBox;
use core::{
    alloc::Layout,
    hint::unreachable_unchecked,
    marker::{PhantomData, PhantomPinned},
    mem::MaybeUninit,
    pin::Pin,
    ptr::NonNull,
};

use elain::{Align, Alignment};

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

/// A raw storage, where object is stored in place.
///
/// Object size and alignment must fit, e.g. be lesser or equal to the generic parameter.
/// This condition is enforced by a constant assertion, which triggers at build time
/// (it is not triggered by **cargo check**).
#[derive(Debug)]
#[repr(C)]
pub struct Raw<const SIZE: usize, const ALIGN: usize = { align_of::<usize>() }>
where
    Align<ALIGN>: Alignment,
{
    data: MaybeUninit<[u8; SIZE]>,
    _align: Align<ALIGN>,
    _not_send_sync: PhantomData<*mut ()>,
    _pinned: PhantomPinned,
}

impl<const SIZE: usize, const ALIGN: usize> Raw<SIZE, ALIGN>
where
    Align<ALIGN>: Alignment,
{
    pub const fn can_store<T>() -> bool {
        size_of::<T>() <= SIZE && align_of::<T>() <= ALIGN
    }

    /// # Safety
    ///
    /// `data` must have size and alignment lesser or equal to the generic parameters.
    const unsafe fn new_unchecked<T>(data: T) -> Self {
        let mut raw = Self {
            data: MaybeUninit::uninit(),
            _align: Align::NEW,
            _not_send_sync: PhantomData,
            _pinned: PhantomPinned,
        };
        // SAFETY: function contract guarantees that `raw.data` size and alignment
        // matches `data` ones; alignment is obtained through `_align` field and `repr(C)`
        unsafe { raw.data.as_mut_ptr().cast::<T>().write(data) };
        raw
    }

    pub const fn new<T>(data: T) -> Self {
        const { assert!(Self::can_store::<T>()) };
        // SAFETY: assertion above ensures function contract
        unsafe { Self::new_unchecked::<T>(data) }
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
        NonNull::from(&self.data).cast()
    }
    fn ptr_mut(&mut self) -> NonNull<()> {
        NonNull::from(&mut self.data).cast()
    }
    unsafe fn drop_in_place(&mut self, _layout: Layout) {}
}

/// A type-erased [`Box`](StdBox).
#[cfg(feature = "alloc")]
#[derive(Debug)]
pub struct Box(NonNull<()>);

#[cfg(feature = "alloc")]
impl Box {
    pub fn new_box<T>(data: StdBox<T>) -> Self {
        Self(NonNull::new(StdBox::into_raw(data).cast()).unwrap())
    }
}

// SAFETY: `ptr`/`ptr_mut` return a pointer to the stored data.
#[cfg(feature = "alloc")]
unsafe impl Storage for Box {
    fn new<T>(data: T) -> Self {
        Self::new_box(StdBox::new(data))
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
#[derive(Debug)]
pub struct RawOrBox<const SIZE: usize, const ALIGN: usize = { align_of::<usize>() }>(
    RawOrBoxInner<SIZE, ALIGN>,
)
where
    Align<ALIGN>: Alignment;

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(feature = "alloc")]
impl<const SIZE: usize, const ALIGN: usize> RawOrBox<SIZE, ALIGN>
where
    Align<ALIGN>: Alignment,
{
    pub const fn new_raw<T>(data: T) -> Self {
        Self(RawOrBoxInner::Raw(Raw::new(data)))
    }

    pub fn new_box<T>(data: StdBox<T>) -> Self {
        if Raw::<SIZE, ALIGN>::can_store::<T>() {
            Self::new(*data)
        } else {
            Self(RawOrBoxInner::Box(Box::new_box(data)))
        }
    }
}

// SAFETY: Both `Raw` and `Box` implements `Storage`
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
            RawOrBoxInner::Raw(s) if Raw::<SIZE, ALIGN>::can_store::<T>() => unsafe { s.as_ref() },
            #[cfg(feature = "alloc")]
            RawOrBoxInner::Box(s) if !Raw::<SIZE, ALIGN>::can_store::<T>() => unsafe { s.as_ref() },
            _ => unsafe { unreachable_unchecked() },
        }
    }
    unsafe fn as_mut<T>(&mut self) -> &mut T {
        match &mut self.0 {
            RawOrBoxInner::Raw(s) if Raw::<SIZE, ALIGN>::can_store::<T>() => unsafe { s.as_mut() },
            #[cfg(feature = "alloc")]
            RawOrBoxInner::Box(s) if !Raw::<SIZE, ALIGN>::can_store::<T>() => unsafe { s.as_mut() },
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
    use core::mem;

    use elain::{Align, Alignment};

    use crate::{DynStorage, storage::Storage};

    #[crate::dyn_storage(crate = crate)]
    trait Test {}
    impl Test for () {}
    impl<const N: usize> Test for [u8; N] {}
    impl Test for u64 {}
    type TestStorage<'__dyn, S> = DynStorage<dyn Test + '__dyn, S>;

    #[test]
    fn raw_alignment() {
        fn check_alignment<const ALIGN: usize>()
        where
            Align<ALIGN>: Alignment,
        {
            let storages = [(); 2].map(TestStorage::<super::Raw<0, ALIGN>>::new);
            for s in &storages {
                assert!(s.inner.ptr().cast::<Align<ALIGN>>().is_aligned());
            }
            const { assert!(ALIGN < 2048) };
            assert!(
                storages
                    .iter()
                    .any(|s| !s.inner.ptr().cast::<Align<2048>>().is_aligned())
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
            let storage = TestStorage::<super::RawOrBox<8>>::new(array);
            assert!(variant(&storage.inner));
            assert_eq!(
                unsafe { storage.inner.ptr().cast::<[u8; N]>().read() },
                array
            );
        }
        check_variant::<4>(|s| matches!(s.0, super::RawOrBoxInner::Raw(_)));
        check_variant::<64>(|s| matches!(s.0, super::RawOrBoxInner::Box(_)));

        let storage = TestStorage::<super::RawOrBox<8, 1>>::new(0u64);
        assert!(matches!(storage.inner.0, super::RawOrBoxInner::Box(_)));
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
            let storage = TestStorage::<S>::new(SetDropped(&mut dropped));
            assert!(!*unsafe { storage.inner.ptr().cast::<SetDropped>().as_ref() }.0);
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
            drop(TestStorage::<S>::new(()));
        }
        check_dst::<super::Raw<{ size_of::<SetDropped>() }, { align_of::<SetDropped>() }>>();
        #[cfg(feature = "alloc")]
        check_dst::<super::Box>();
        #[cfg(feature = "alloc")]
        check_dst::<super::RawOrBox<{ size_of::<SetDropped>() }>>();
        #[cfg(feature = "alloc")]
        check_dst::<super::RawOrBox<0>>();
    }
}
