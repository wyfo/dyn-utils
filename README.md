# dyn-utils

*This crate is at an early stage of development and is not yet released. Despite being
extensively tested with [miri], it contains a lot of unsafe code, and some uncaught 
unsoundness may still remain.*

*Documentation is available at <https://wyfo.github.io/dyn-utils/>. You can test the crate 
using a git dependency:*

```toml
dyn-utils = { git = "https://github.com/wyfo/dyn-utils" }
```

A utility library for working with [trait objects].

Trait objects (i.e. `dyn Trait`) are unsized and therefore need to be stored in a container
such as `Box`. This crate provides `DynObject`, a container for trait objects with
generic storage.

`storage::Raw` stores objects in place, making `DynObject<dyn Trait, storage::Raw>`
allocation-free. On the other hand, `storage::RawOrBox` falls back to an allocated `Box` if
the object is too large to fit in place.

Avoiding one allocation makes `DynObject` a good alternative to `Box` when writing a
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

This crate also provides a `dyn_trait` proc-macro to achieve the same result as above:

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

- Heapless storage for arbitrary trait objects. It can be used to make an async trait
  dyn-compatible, but also to store dyn-compatible trait objects in
  resource constrained environments.
- Compile-time assertions for heapless storage of trait objects.
- Fallback to allocated storage if a trait object does not fit in place.
- Synchronous execution-path optimization for asynchronous methods, bringing significant
  performance improvements.
- Clean ergonomics with procedural macros.
- [Better performance](benches/README.md) than most alternatives.

## Comparisons with other similar projects

### [async-trait](https://crates.io/crates/async-trait)

*async-trait* is the reference solution for providing dyn-compatible traits with async
functions. However, it works by boxing the returned future, which adds a performance
penalty. The `async-trait` macro also rewrites the trait in place, making the allocation
mandatory even without dynamic dispatch.

### [dynosaur](https://crates.io/crates/dynosaur)

Contrary to `async-trait`, `dynosaur` generates a dyn-compatible trait prefixed with
`Dyn`, but it still relies on boxing the returned future.

### [dynify](https://crates.io/crates/dynify)

`dynify` also generates a dyn-compatible trait, but its methods return a special handle
that must be initialized with a provided storage, either stack-based or heap-based. It
avoids the allocation penalty at the cost of ergonomics, as it requires significant
scaffolding just to await a returned future.

Moreover, while it allows the use of stack-based storage, `dynify` relies on runtime
checks and provides no reliable way to ensure that a returned future fits on the stack.
This makes it unsuitable for resource-constrained use cases.

The only thing that `dyn-utils` does not support is reusing the same allocated storage
for multiple dynamic calls[^1].

### [stackfuture](https://crates.io/crates/stackfuture)

`stackfuture` is not concerned with traits; it is simply a type used to store an unsized
future. It is conceptually similar to `DynObject<dyn Future<T>, Raw<SIZE>>`: both 
provide compile-time assertions, making them suitable for resource-constrained 
environments. If a procedural macro crate had been built around `stackfuture`, it would 
likely look similar to `dyn-utils`.

However, `stackfuture` does not support arbitrary traits with arbitrary bounds, only
`Future + Send`. It also uses an inline vtable (functions are stored directly in the
struct), whereas `DynObject` uses a more generic static vtable reference. Another
difference is that `stackfuture` always performs a virtual drop of the concrete future,
while `DynObject` implements the same optimization as regular trait objects, avoiding a
virtual call when it is not needed[^2].

## Disclaimer

The original experiment behind this crate was
[dyn-fn](https://github.com/wyfo/dyn-fn), a project I made for an intern I was supervising.
The requirement was to support asynchronous callbacks in a no-allocation environment.
At the time, I was only aware of `async-trait`, so I crafted my own solution.

I later noticed that the principles behind `dyn-fn` could be generalized with procedural
macros, which led to this project. I only discovered `dynify`, and with it the broader
alternative ecosystem, when I was looking for a name for the crate — yes, I initially wanted
to name it *dynify*.

The comparison section above shows that this crate still adds value to the ecosystem,
which is why I continued working on it. But to be honest, I wouldn't have started this
project if I had known about `stackfuture` before.

[^1]: In practice, it is not hard to support allocated storage reuse in `dyn-utils`, but
it has several drawbacks: ergonomics (as it requires passing the `DynObject` as an
argument), performance (my tests show that returning a `DynObject` is faster than passing
it as an argument), increased API complexity, and, in any case, if a future is too large
to fit on the stack, the allocation cost is likely negligible.
[^2]: As of today, async blocks and futures returned by `async fn` always have a mandatory
destructor, even if they do not capture any droppable variables. `stackfuture` is therefore
more optimized for this use case, until `rustc` is improved on the subject.

[trait objects]: https://doc.rust-lang.org/std/keyword.dyn.html
[dyn-compatible]: https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility
