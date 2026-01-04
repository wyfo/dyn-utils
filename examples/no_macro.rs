use dyn_utils::DynStorage;
use futures::FutureExt;

trait Callback {
    fn call(&self, arg: &str) -> impl Future<Output = ()> + Send
    where
        Self: Sized;
}

trait CallbackDyn<S: dyn_utils::storage::Storage = dyn_utils::DefaultStorage> {
    fn call<'a>(&'a self, arg: &'a str) -> DynStorage<dyn Future<Output = ()> + Send + 'a, S>;
}

impl<T: Callback, S: dyn_utils::storage::Storage> CallbackDyn<S> for T {
    fn call<'a>(&'a self, arg: &'a str) -> DynStorage<dyn Future<Output = ()> + Send + 'a, S> {
        DynStorage::new(self.call(arg))
    }
}

struct HelloCallback;
impl Callback for HelloCallback {
    async fn call(&self, arg: &str) {
        println!("Hello {arg}!");
    }
}

fn main() {
    let callback: Box<dyn CallbackDyn> = Box::new(HelloCallback);
    callback.call("world").now_or_never(); // prints "Hello world!"
}
