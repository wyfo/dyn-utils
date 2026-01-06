use std::{
    future::ready,
    hint::black_box,
    mem::MaybeUninit,
    pin::{Pin, pin},
    task::{Context, Poll, Waker},
};

use divan::Bencher;
use dyn_utils::{DynObject, storage::Raw};
use dynify::Dynify;
use futures::future::OptionFuture;
use stackfuture::StackFuture;

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
trait Trait {
    #[dyn_trait(try_sync)]
    async fn future(&self, s: &str) -> usize {
        s.len()
    }
    fn future_with_storage<'a, 'storage>(
        &'a self,
        s: &'a str,
        storage: Pin<&'storage mut Option<DynObject<dyn Future<Output = usize> + 'a>>>,
    ) -> Pin<&'storage mut (dyn Future<Output = usize> + 'a)> {
        DynObject::insert_pinned(storage, self.future(s))
    }
    fn future_with_storage_option_future<'a, 'storage>(
        &'a self,
        s: &'a str,
        mut storage: Pin<&'storage mut OptionFuture<DynObject<dyn Future<Output = usize> + 'a>>>,
    ) {
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

trait Trait4 {
    fn future<'a>(&'a self, s: &'a str) -> StackFuture<'a, usize, 128>;
}

#[dynosaur::dynosaur(DynTrait5 = dyn(box) Trait5)]
trait Trait5 {
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

struct NoDrop;
impl Trait for NoDrop {
    fn future(&self, s: &str) -> impl Future<Output = usize> {
        ready(s.len())
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

impl Trait4 for () {
    fn future<'a>(&'a self, s: &'a str) -> StackFuture<'a, usize, 128> {
        StackFuture::from(async move { s.len() })
    }
}

impl Trait4 for NoDrop {
    fn future<'a>(&'a self, s: &'a str) -> StackFuture<'a, usize, 128> {
        StackFuture::from(ready(s.len()))
    }
}

impl Trait5 for () {
    async fn future(&self, s: &str) -> usize {
        s.len()
    }
}

#[divan::bench]
fn dyn_utils_future(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait>);
    b.bench_local(|| now_or_never!(test.future("test")));
}

#[divan::bench]
fn dyn_utils_future_no_alloc(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait<Raw<128>>>);
    b.bench_local(|| now_or_never!(test.future("test")));
}

#[divan::bench]
fn dyn_utils_future_no_drop(b: Bencher) {
    let test = black_box(Box::new(NoDrop) as Box<dyn DynTrait>);
    b.bench_local(|| now_or_never!(test.future("test")));
}

#[divan::bench]
fn dyn_utils_future_try_sync(b: Bencher) {
    let test = black_box(Box::new(Sync) as Box<dyn DynTrait>);
    b.bench_local(|| now_or_never!(test.future_try_sync("test")));
}

#[divan::bench]
fn dyn_utils_future_try_sync_fallback(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait>);
    b.bench_local(|| now_or_never!(test.future_try_sync("test")));
}

#[divan::bench]
fn dyn_utils_future_with_storage(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait>);
    b.bench_local(|| {
        let storage = pin!(None);
        now_or_never!(test.future_with_storage("test", storage))
    });
}

#[divan::bench]
fn dyn_utils_future_with_storage_option_future(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait>);
    b.bench_local(|| {
        let mut storage: Pin<&mut OptionFuture<_>> = pin!(None.into());
        test.future_with_storage_option_future("test", storage.as_mut());
        now_or_never!(storage).map(Option::unwrap)
    });
}

#[divan::bench]
fn dyn_utils_iter(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait>);
    b.bench_local(|| test.iter().count());
}

#[divan::bench]
fn dynify_future(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait2>);
    b.bench_local(|| {
        let mut stack = [MaybeUninit::<u8>::uninit(); 128];
        let mut heap = Vec::<MaybeUninit<u8>>::new();
        let init = test.future("test");
        now_or_never!(init.init2(&mut stack, &mut heap));
    });
}

#[divan::bench]
fn dynify_future_no_alloc(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn DynTrait2>);
    b.bench_local(|| {
        let mut stack = [MaybeUninit::<u8>::uninit(); 128];
        let init = test.future("test");
        now_or_never!(init.init(&mut stack));
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
fn async_trait_future(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn Trait3>);
    b.bench_local(|| now_or_never!(test.future("test")));
}

#[divan::bench]
fn stackfuture_future(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn Trait4>);
    b.bench_local(|| now_or_never!(test.future("test")));
}

#[divan::bench]
fn stackfuture_future_no_drop(b: Bencher) {
    let test = black_box(Box::new(NoDrop) as Box<dyn Trait4>);
    b.bench_local(|| now_or_never!(test.future("test")));
}

#[divan::bench]
fn dynosaur_future(b: Bencher) {
    let test = black_box(DynTrait5::from_box(Box::new(())));
    b.bench_local(|| now_or_never!(test.future("test")));
}

fn main() {
    divan::main();
}
