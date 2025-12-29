#[cfg(feature = "alloc")]
use alloc::boxed::Box as StdBox;
use core::{
    alloc::Layout,
    fmt,
    marker::{PhantomData, PhantomPinned},
    mem::MaybeUninit,
    ptr::NonNull,
};

use elain::{Align, Alignment};

use crate::{private, Storage};

pub struct DynStorage<
    'a,
    S: Storage,
    VT: private::StorageVTable,
    T: ?Sized = <VT as private::StorageVTable>::DynTrait,
> {
    inner: S,
    vtable: &'static VT,
    _lifetime: PhantomData<&'a mut T>,
}

unsafe impl<S: Storage, VT: private::StorageVTable, T: ?Sized> Send for DynStorage<'_, S, VT, T> {}

unsafe impl<S: Storage, VT: private::StorageVTable, T: ?Sized> Sync for DynStorage<'_, S, VT, T> {}

impl<S: Storage, VT: private::StorageVTable, T: ?Sized> DynStorage<'_, S, VT, T> {
    /// # Safety
    ///
    /// `vtable.drop_vtable()` must match the storage `inner` and the data stored inside.
    pub const unsafe fn from_raw_parts(inner: S, vtable: &'static VT) -> Self {
        Self {
            inner,
            vtable,
            _lifetime: PhantomData,
        }
    }

    #[doc(hidden)]
    pub fn vtable(&self) -> &'static VT {
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
}

impl<S: Storage, VT: private::StorageVTable, T: ?Sized> Drop for DynStorage<'_, S, VT, T> {
    fn drop(&mut self) {
        if let Some(drop_inner) = self.vtable.drop_in_place() {
            // SAFETY: the storage data is no longer accessed after the call,
            // and is matched by the vtable as per function contract.
            unsafe { drop_inner(self.inner.ptr_mut()) };
        }
        // SAFETY: the storage data is no longer accessed after the call,
        // and is matched by the vtable as per function contract.
        unsafe { self.inner.drop_in_place(self.vtable.layout()) };
    }
}

impl<S: Storage + fmt::Debug, VT: private::StorageVTable + fmt::Debug, T: ?Sized> fmt::Debug
    for DynStorage<'_, S, VT, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynStorage")
            .field("inner", &self.inner)
            .field("vtable", &self.vtable)
            .finish()
    }
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
    fn try_new<T>(data: T) -> Result<Self, T> {
        if !Self::can_store::<T>() {
            return Err(data);
        }
        Ok(unsafe { Self::new_unchecked(data) })
    }
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
    fn try_new<T>(data: T) -> Result<Self, T> {
        Ok(Self::new(data))
    }
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

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(test)]
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use core::{marker::PhantomData, mem, mem::ManuallyDrop};

    use elain::{Align, Alignment};

    use crate::{
        private::{DynVTable, StorageMoved},
        storage::DynStorage,
        Storage,
    };

    type TestStorage<'a, S> = DynStorage<'a, S, DynVTable>;
    impl<'a, S: Storage> TestStorage<'a, S> {
        fn new_test<T: 'a>(data: T) -> Self {
            Self {
                inner: S::new(data),
                vtable: &const { DynVTable::new::<S, T>() },
                _lifetime: PhantomData,
            }
        }
    }

    #[cfg(all(feature = "alloc", feature = "either"))]
    type RawOrBox<const SIZE: usize, const ALIGN: usize = { align_of::<usize>() }> =
        either::Either<super::Raw<SIZE, ALIGN>, super::Box>;

    #[test]
    fn raw_alignment() {
        fn check_alignment<const ALIGN: usize>()
        where
            Align<ALIGN>: Alignment,
        {
            let storages = [(); 2].map(TestStorage::<super::Raw<0, ALIGN>>::new_test);
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

    #[cfg(all(feature = "alloc", feature = "either"))]
    #[test]
    fn either() {
        use either::*;
        fn check_variant<const N: usize>(variant: impl Fn(&RawOrBox<8>) -> bool) {
            let array = core::array::from_fn::<u8, N, _>(|i| i as u8);
            let storage = TestStorage::<RawOrBox<8>>::new_test(array);
            assert!(variant(&storage.inner));
            assert_eq!(
                unsafe { storage.inner.ptr().cast::<[u8; N]>().read() },
                array
            );
        }
        check_variant::<4>(|s| matches!(s, Left(_)));
        check_variant::<64>(|s| matches!(s, Right(_)));

        let storage = TestStorage::<RawOrBox<8, 1>>::new_test(0u64);
        assert!(matches!(storage.inner, Right(_)));
    }

    struct SetDropped<'a>(&'a mut bool);
    impl Drop for SetDropped<'_> {
        fn drop(&mut self) {
            assert!(!mem::replace(self.0, true));
        }
    }

    #[test]
    fn storage_drop() {
        fn check_drop<S: Storage>() {
            let mut dropped = false;
            let storage = TestStorage::<S>::new_test(SetDropped(&mut dropped));
            assert!(!*unsafe { storage.inner.ptr().cast::<SetDropped>().as_ref() }.0);
            drop(storage);
            assert!(dropped);
        }
        check_drop::<super::Raw<{ size_of::<SetDropped>() }, { align_of::<SetDropped>() }>>();
        #[cfg(feature = "alloc")]
        check_drop::<super::Box>();
        #[cfg(all(feature = "alloc", feature = "either"))]
        check_drop::<RawOrBox<{ size_of::<SetDropped>() }>>();
        #[cfg(all(feature = "alloc", feature = "either"))]
        check_drop::<RawOrBox<0>>();
    }

    #[test]
    fn storage_drop_moved() {
        fn check_drop_moved<S: Storage>() {
            let mut dropped = false;
            let mut storage =
                ManuallyDrop::new(TestStorage::<S>::new_test(SetDropped(&mut dropped)));
            let moved = unsafe { StorageMoved::<S, SetDropped>::new(&mut storage.inner) };
            unsafe { drop(moved.read()) };
            drop(moved);
            assert!(dropped);
        }
        check_drop_moved::<super::Raw<{ size_of::<SetDropped>() }, { align_of::<SetDropped>() }>>();
        #[cfg(feature = "alloc")]
        check_drop_moved::<super::Box>();
        #[cfg(all(feature = "alloc", feature = "either"))]
        check_drop_moved::<RawOrBox<{ size_of::<SetDropped>() }>>();
        #[cfg(all(feature = "alloc", feature = "either"))]
        check_drop_moved::<RawOrBox<0>>();
    }

    #[test]
    fn storage_dst() {
        fn check_dst<S: Storage>() {
            drop(TestStorage::<S>::new_test(()));
        }
        check_dst::<super::Raw<{ size_of::<SetDropped>() }, { align_of::<SetDropped>() }>>();
        #[cfg(feature = "alloc")]
        check_dst::<super::Box>();
        #[cfg(all(feature = "alloc", feature = "either"))]
        check_dst::<RawOrBox<{ size_of::<SetDropped>() }>>();
        #[cfg(all(feature = "alloc", feature = "either"))]
        check_dst::<RawOrBox<0>>();
    }
}
