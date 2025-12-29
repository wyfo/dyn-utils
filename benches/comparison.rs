use std::{
    hint::black_box,
    mem::MaybeUninit,
    pin::pin,
    task::{Context, Poll, Waker},
};

use divan::Bencher;
use dynify::Dynify;

// `futures::future::FutureExt::now_or_never` was not inlined, and it was messing
// the results up, with `dyn_async_trait` as fast as `dyn_async_fn`
pub trait FutureExt: Future + Sized {
    #[inline(always)]
    fn now_or_never(self) -> Option<Self::Output> {
        match pin!(self).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(x) => Some(x),
            _ => None,
        }
    }
}

impl<F: Future> FutureExt for F {}

#[dyn_utils::with_storage]
trait AsyncFn {
    async fn call(&self, s: &str) -> usize;
    fn iter(&self) -> impl Iterator<Item = usize>;
}

#[dynify::dynify]
trait AsyncFn2 {
    async fn call(&self, s: &str) -> usize;
    fn iter(&self) -> impl Iterator<Item = usize>;
}

#[async_trait::async_trait]
trait AsyncFn3 {
    async fn call(&self, s: &str) -> usize;
}

struct Foo;
impl AsyncFn for Foo {
    async fn call(&self, s: &str) -> usize {
        s.len()
    }
    fn iter(&self) -> impl Iterator<Item = usize> {
        [1, 2, 3, 4].into_iter()
    }
}

impl AsyncFn2 for Foo {
    async fn call(&self, s: &str) -> usize {
        s.len()
    }
    fn iter(&self) -> impl Iterator<Item = usize> {
        [1, 2, 3, 4].into_iter()
    }
}

#[async_trait::async_trait]
impl AsyncFn3 for Foo {
    async fn call(&self, s: &str) -> usize {
        s.len()
    }
}

#[divan::bench]
fn dyn_utils_async(b: Bencher) {
    let foo = black_box(Box::new(Foo) as Box<dyn AsyncFnWithStorage>);
    b.bench_local(|| {
        let storage = pin!(None);
        foo.call_with_storage("test", storage).now_or_never()
    });
}

#[divan::bench]
fn dyn_utils_iter(b: Bencher) {
    let foo = black_box(Box::new(Foo) as Box<dyn AsyncFnWithStorage>);
    b.bench_local(|| {
        let mut storage = None;
        foo.iter_with_storage(&mut storage).count()
    });
}

#[divan::bench]
fn dynify(b: Bencher) {
    let foo = black_box(Box::new(Foo) as Box<dyn DynAsyncFn2>);
    b.bench_local(|| {
        let mut stack = [MaybeUninit::<u8>::uninit(); 128];
        let mut heap = Vec::<MaybeUninit<u8>>::new();
        let init = foo.call("test");
        let fut = init.init2(&mut stack, &mut heap);
        fut.now_or_never()
    });
}

#[divan::bench]
fn dynify_iter(b: Bencher) {
    let foo = black_box(Box::new(Foo) as Box<dyn DynAsyncFn2>);
    b.bench_local(|| {
        let mut stack = [MaybeUninit::<u8>::uninit(); 128];
        let mut heap = Vec::<MaybeUninit<u8>>::new();
        let init = foo.iter();
        let mut iter = init.init2(&mut stack, &mut heap);
        (&mut *iter).count()
    });
}

#[divan::bench]
fn async_trait(b: Bencher) {
    let foo = black_box(Box::new(Foo) as Box<dyn AsyncFn3>);
    b.bench_local(|| foo.call("test").now_or_never());
}

fn main() {
    divan::main();
}
