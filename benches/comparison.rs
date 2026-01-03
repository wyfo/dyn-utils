use std::{
    hint::black_box,
    mem::MaybeUninit,
    pin::pin,
    task::{Context, Poll, Waker},
};

use divan::Bencher;
use dynify::Dynify;

// `futures::future::FutureExt::now_or_never` is not properly inlined
macro_rules! now_or_never {
    ($future:expr) => {
        match pin!($future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(x) => Some(x),
            _ => None,
        }
    };
}

#[dyn_utils::dyn_compatible]
trait Trait {
    #[dyn_utils(try_sync)]
    async fn future(&self, s: &str) -> usize {
        s.len()
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
    let test = black_box(Box::new(()) as Box<dyn TraitDyn>);
    b.bench_local(|| now_or_never!(test.future("test")));
}

#[divan::bench]
fn dyn_utils_try_sync(b: Bencher) {
    let test = black_box(Box::new(Sync) as Box<dyn TraitDyn>);
    b.bench_local(|| now_or_never!(test.future_try_sync("test")));
}

#[divan::bench]
fn dyn_utils_try_sync_fallback(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn TraitDyn>);
    b.bench_local(|| now_or_never!(test.future_try_sync("test")));
}

#[divan::bench]
fn dyn_utils_iter(b: Bencher) {
    let test = black_box(Box::new(()) as Box<dyn TraitDyn>);
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
