use dyn_utils::object::DynObject;
use futures::FutureExt;

trait Callback {
    fn call(&self, arg: &str) -> impl Future<Output = ()> + Send;
}

trait DynCallback<S: dyn_utils::storage::Storage = dyn_utils::storage::DefaultStorage> {
    fn call<'a>(&'a self, arg: &'a str) -> DynObject<dyn Future<Output = ()> + Send + 'a, S>;
}

impl<T: Callback, S: dyn_utils::storage::Storage> DynCallback<S> for T {
    fn call<'a>(&'a self, arg: &'a str) -> DynObject<dyn Future<Output = ()> + Send + 'a, S> {
        DynObject::new(self.call(arg))
    }
}

struct HelloCallback;
impl Callback for HelloCallback {
    async fn call(&self, arg: &str) {
        println!("Hello {arg}!");
    }
}

fn main() {
    let callback: Box<dyn DynCallback> = Box::new(HelloCallback);
    callback.call("world").now_or_never(); // prints "Hello world!"
}
