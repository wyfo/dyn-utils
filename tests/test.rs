#![allow(unused)]

use futures::FutureExt;

macro_rules! nothing {
    () => {};
}

#[dyn_utils::dyn_compatible]
trait Test {
    type Result;
    fn method(&self) -> Self::Result;
    async fn future<'a>(&self, s: &'a str) -> &'a str;
    #[allow(clippy::needless_lifetimes)] // test non-captured generic lifetime
    fn future_send<'a>(&'_ self, s: &'a str) -> impl Future<Output = usize> + Send + use<Self>;
    async fn empty(&self);
    nothing!();
}

impl Test for () {
    type Result = usize;
    fn method(&self) -> Self::Result {
        42
    }
    async fn future<'a>(&self, s: &'a str) -> &'a str {
        s
    }
    fn future_send(&self, s: &str) -> impl Future<Output = usize> + Send + use<> {
        let len = s.len();
        async move { len }
    }
    async fn empty(&self) {}
}

#[test]
fn test() {
    let test = Box::new(()) as Box<dyn DynTest<Result = usize>>;
    test.empty().now_or_never().unwrap();
    assert_eq!(test.method(), 42);
    assert_eq!(test.future("test").now_or_never(), Some("test"));
    assert_eq!(test.future_send("test").now_or_never(), Some(4));
}
