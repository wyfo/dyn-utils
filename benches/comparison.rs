use std::{
    hint::black_box,
    mem::MaybeUninit,
    pin::{Pin, pin},
    task::{Context, Poll, Waker},
};

use divan::Bencher;
use dyn_utils::DynObject;
use dynify::Dynify;
use futures::future::OptionFuture;

// `futures::future::FutureExt::now_or_never` is not properly inlined
macro_rules! now_or_never {
    ($future:expr) => {
        match pin!($future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(x) => Some(x),
            _ => None,
        }
    };
}

#[dyn_utils::dyn_trait]
trait Trait<Storage: dyn_utils::storage::Storage = dyn_utils::DefaultStorage> {
    #[dyn_trait(try_sync)]
    async fn future(&self, s: &str) -> usize {
        s.len()
    }
    fn future_with_storage<'a, 'storage>(
        &'a self,
        s: &'a str,
        storage: Pin<&'storage mut Option<DynObject<dyn Future<Output = usize> + 'a, Storage>>>,
    ) -> Pin<&'storage mut (dyn Future<Output = usize> + 'a)>
    where
        Storage: 'a,
    {
        DynObject::insert_pinned(storage, self.future(s))
    }
    fn future_with_storage_option_future<'a, 'storage>(
        &'a self,
        s: &'a str,
        mut storage: Pin<
            &'storage mut OptionFuture<DynObject<dyn Future<Output = usize> + 'a, Storage>>,
        >,
    ) where
        Storage: 'a,
    {
        storage.set(Some(DynObject::new(self.future(s))).into());
    }
    fn iter(&self) -> impl Iterator<Item = usize> {
        [1, 2, 3, 4].into_iter()
    }
}

#[dynify::dynify]
trait Trait2 {
    async fn future(&self, s: &str) -> usize;
    fn iter(&self) -> impl Iterator<Item = usize>;
}

#[async_trait::async_trait]
trait Trait3 {
    async fn future(&self, s: &str) -> usize;
}

impl Trait for () {}

struct Sync;
impl Trait for Sync {
    #[dyn_utils::sync]
    async fn future(&self, s: &str) -> usize {
        s.len()
    }
}

impl Trait2 for () {
    async fn future(&self, s: &str) -> usize {
        s.len()
    }
    fn iter(&self) -> impl Iterator<Item = usize> {
        [1, 2, 3, 4].into_iter()
    }
}

#[async_trait::async_trait]
impl Trait3 for () {
    async fn future(&self, s: &str) -> usize {
        s.len()
    }
}

#[divan::bench]
fn dyn_utils_async(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait>);
    b.bench_local(|| now_or_never!(test.future("test")));
}

#[divan::bench]
fn dyn_utils_async_with_storage(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait>);
    b.bench_local(|| {
        let storage = pin!(None);
        now_or_never!(test.future_with_storage("test", storage))
    });
}

#[divan::bench]
fn dyn_utils_async_with_storage_option_future(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait>);
    b.bench_local(|| {
        let mut storage: Pin<&mut OptionFuture<_>> = pin!(None.into());
        test.future_with_storage_option_future("test", storage.as_mut());
        now_or_never!(storage).map(Option::unwrap)
    });
}

#[divan::bench]
fn dyn_utils_try_sync(b: Bencher) {
    let test = black_box(Box::new(Sync) as Box<dyn DynTrait>);
    b.bench_local(|| now_or_never!(test.future_try_sync("test")));
}

#[divan::bench]
fn dyn_utils_try_sync_fallback(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait>);
    b.bench_local(|| now_or_never!(test.future_try_sync("test")));
}

#[divan::bench]
fn dyn_utils_iter(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait>);
    b.bench_local(|| test.iter().count());
}

#[divan::bench]
fn dynify(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait2>);
    b.bench_local(|| {
        let mut stack = [MaybeUninit::<u8>::uninit(); 128];
        let mut heap = Vec::<MaybeUninit<u8>>::new();
        let init = test.future("test");
        now_or_never!(init.init2(&mut stack, &mut heap));
    });
}

#[divan::bench]
fn dynify_iter(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait2>);
    b.bench_local(|| {
        let mut stack = [MaybeUninit::<u8>::uninit(); 128];
        let mut heap = Vec::<MaybeUninit<u8>>::new();
        let init = test.iter();
        let mut iter = init.init2(&mut stack, &mut heap);
        (&mut *iter).count()
    });
}

#[divan::bench]
fn async_trait(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn Trait3>);
    b.bench_local(|| now_or_never!(test.future("test")));
}

fn main() {
    divan::main();
}
