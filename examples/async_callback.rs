use dyn_utils::DynStorage;
use futures::FutureExt;

#[dyn_utils::dyn_trait]
#[dyn_utils(dyn_storage)]
trait Callback {
    #[dyn_utils(try_sync, Send)]
    async fn call(&self, arg: &str);
}

struct HelloCallback;
impl Callback for HelloCallback {
    #[dyn_utils::sync]
    async fn call(&self, arg: &str) {
        println!("Hello {arg}!");
    }
}

fn main() {
    let callback: DynStorage<dyn CallbackDyn> = DynStorage::new(HelloCallback); // no allocation
    callback.call_try_sync("world").now_or_never(); // prints "Hello world!"
}
