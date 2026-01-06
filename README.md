# dyn-utils

*This crate is at an early stage of development, and is not yet released. Despite being extensively tested with [miri], it contains a lot of unsafe code, and there may remain some uncaught unsoundness.* 

*Documentation is available at [https://wyfo.github.io/dyn-fn/](https://wyfo.github.io/dyn-fn/). You can test the crate using a git dependency:*
```toml
dyn-fn = { git = "https://github.com/wyfo/dyn-fn" }
```

A utility library for working with [trait objects].

Trait objects, i.e. `dyn Trait`, are unsized and requires to be stored in a container
like `Box`. This crate provides `DynObject`, a container for trait object with a
generic storage.

`storage::Raw` stores object in place, making `DynObject<dyn Trait, storage::Raw>`
allocation-free. On the other hand, `storage::RawOrBox` falls back to an allocated `Box` if
the object is too big to fit in place.

Saving one allocation makes `DynObject` a good alternative to `Box` when it comes to write a
[dyn-compatible] version of a trait with return-position `impl Trait`, such as async methods.

## Examples

```rust
use dyn_utils::DynObject;

trait Callback {
    fn call(&self, arg: &str) -> impl Future<Output = ()> + Send;
}

// Dyn-compatible version
trait DynCallback {
    fn call<'a>(&'a self, arg: &'a str) -> DynObject<dyn Future<Output = ()> + Send + 'a>;
}

impl<T: Callback> DynCallback for T {
    fn call<'a>(&'a self, arg: &'a str) -> DynObject<dyn Future<Output = ()> + Send + 'a> {
        DynObject::new(self.call(arg))
    }
}

async fn exec_callback(callback: &dyn DynCallback) {
    callback.call("Hello world!").await;
}
```

This crate also provides `dyn_trait` proc-macro to do the same as above:

```rust
#[dyn_utils::dyn_trait] // generates `DynCallback` trait
trait Callback {
    fn call(&self, arg: &str) -> impl Future<Output = ()> + Send;
}

async fn exec_callback(callback: &dyn DynCallback) {
    callback.call("Hello world!").await;
}
```

## Features

- Heapless storage for arbitrary trait objects. It can be used to make an async trait dyn-compatible, but also to store the dyn-compatible trait object in resource constrained environment.
- Compile-time assertions for heapless storing of trait objects.
- Fallback to allocated storage if a trait object doesn't fit in place.
- Synchronous execution path optimization for asynchronous methods, bringing a significant performance improvement. 
- Clean ergonomics with procedural macros
- [Better performance](benches/README.md) than most alternatives

## Comparisons with other similar projects

#### [async-trait](https://crates.io/crates/async-trait)

*async-trait* is the reference when it comes to provides dyn-compatible traits with async function. However, it works by boxing the returned future, adding a performance penalty. `async-trait` macro also rewrite the trait in place, making the allocation mandatory even without dynamic dispatch.

#### [dynausor](https://crates.io/crates/dynosaur)

Contrary to `async-trait`, `dynausor` generates a dyn-compatible trait prefixed with "Dyn", but still relies on boxing the returned future. 

#### [dynify](https://crates.io/crates/dynify)

`dynify` also generates a dyn-compatible trait, but its method return a special handler that must be initialized with a provided storage, stack/heap based. It avoids allocation penalty, at the cost of ergonomics, as it requires a lot of scaffolding to just await a returned future.
 
However, if it allows to use stack-based storage, `dynify` relies on runtime checks, giving no reliable way to ensure that a returned future fits in the stack. It makes it not suitable for resource-constrained use case. 

The only thing that `dyn-utils` doesn't support is to reuse the same allocated storage for multiple dynamic calls[^1].

#### [stackfuture](https://crates.io/crates/stackfuture)

`stackfuture` doesn't care about traits, it's just a type to store an unsized future, and is in fact quite similar to `DynObject<dyn Future<T>, Raw<SIZE>>`; both provides compile-time assertion, making them suitable for resource-constrained environment. If a proc-macro crate had been built around `stackfuture`, it would have look like `dyn-utils`. 

However, `stackfuture` doesn't support arbitrary traits with arbitrary bounds, only `Future + Send`. It also uses an inline vtable (function are directly stored in the `StackFuture` struct), while `DynObject` uses a more generic `&'static VTable`. Last difference is that `stackfuture` always execute a virtual drop of the concrete future, while `DynObject` implement the same optimization as regular trait object: no virtual call if not needed[^2]. 

## Disclaimer

The original experiment behind this crate was [dyn-fn](https://github.com/wyfo/dyn-fn), a project I made for the intern I was supervising. The requirement was to have asynchronous callbacks in no-alloc environment. At that time, I only knew about `async-trait`, so I crafted my own solution. I also noticed that the principle behind `dyn-fn` could be generalized with some proc-macros, hence this project. I only discovered `dynify`, and with it the whole alternative ecosystem, when I was looking for a name for the crate — yes, I wanted to name it *dynify*.

The comparison section above proves that this crate still add some value to the ecosystem, that's why I continued working on it.

[^1]: Actually, it's not hard to support allocated storage reuse in `dyn-utils`, but it has several drawbacks: ergonomics (as it requires to pass the `DynObject` as argument), performance (my tests shows that returning a `DynObject` is better than passing it in argument), API complexity, and anyway if a future is too big to fit on the stack, the allocation cost may be negligible for it.
[^2]: As of today, async blocks and futures returned by async fn always have a mandatory destructor, even if they don't capture any droppable variables. `stackfuture` is thus more optimized for this use case, until rustc is improved on the subject.

[trait objects]: https://doc.rust-lang.org/std/keyword.dyn.html
[dyn-compatible]: https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility
