use dyn_utils::DynStorage;
use futures::FutureExt;

#[dyn_utils::dyn_trait] // make the trait dyn-compatible
#[dyn_trait(dyn_utils::dyn_storage)] // make the dyn-compatible trait usable with DynStorage
trait Callback {
    #[dyn_trait(try_sync)] // add `call_try_sync` method with synchronous shortcut
    fn call(&self, arg: &str) -> impl Future<Output = ()> + Send;
}

struct HelloCallback;
impl Callback for HelloCallback {
    #[dyn_utils::sync] // make call_try_sync call use synchronous path
    async fn call(&self, arg: &str) {
        println!("Hello {arg}!");
    }
}

fn main() {
    let callback: DynStorage<dyn CallbackDyn> = DynStorage::new(HelloCallback); // no allocation
    callback.call_try_sync("world").now_or_never(); // prints "Hello world!"
}
