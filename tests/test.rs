use std::pin::Pin;

use futures::FutureExt;

macro_rules! nothing {
    () => {};
}

#[dyn_utils::dyn_compatible(Test2)]
trait Test {
    type GAT<T>;
    type Result;
    fn method(&self) -> Self::Result;
    #[dyn_utils(storage = dyn_utils::DefaultStorage, try_sync)]
    async fn future<'a>(&self, s: &'a str) -> &'a str;
    #[dyn_utils(try_sync)]
    async fn future2<'a>(&self, s: &'a str) -> &'a str;
    #[allow(clippy::needless_lifetimes)] // test non-captured generic lifetime
    fn future_send<'a>(&'_ self, s: &'a str) -> impl Future<Output = usize> + Send + use<Self>;
    #[dyn_utils(try_sync)]
    async fn empty(&self);
    fn pinned_self(self: Pin<&mut Self>);
    nothing!();
}

impl Test for () {
    type GAT<T> = ();
    type Result = usize;
    fn method(&self) -> Self::Result {
        42
    }
    async fn future<'a>(&self, s: &'a str) -> &'a str {
        s
    }
    #[dyn_utils::sync]
    async fn future2<'a>(&self, s: &'a str) -> &'a str {
        s
    }
    fn future_send(&self, s: &str) -> impl Future<Output = usize> + Send + use<> {
        let len = s.len();
        async move { len }
    }
    fn pinned_self(self: Pin<&mut Self>) {}
    async fn empty(&self) {}
}

#[test]
fn test() {
    let test = Box::new(()) as Box<dyn Test2<Result = usize>>;
    test.empty().now_or_never().unwrap();
    assert_eq!(test.method(), 42);
    assert_eq!(test.future("test").now_or_never(), Some("test"));
    assert_eq!(test.future_try_sync("test").now_or_never(), Some("test"));
    assert_eq!(test.future2("test").now_or_never(), Some("test"));
    assert_eq!(test.future2_try_sync("test").now_or_never(), Some("test"));
    assert_eq!(test.future_send("test").now_or_never(), Some(4));
}
