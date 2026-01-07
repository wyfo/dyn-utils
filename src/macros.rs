/// Make a trait compatible with `DynObject`.
///
///
///
/// # Arguments
///
/// - `bounds`: Additional bounds, e.g. `Send`, allowing to use `DynObject<dyn Trait + Send>`.
///
/// # Examples
///
/// ```
/// # use std::{pin::Pin, task::{Poll, Context}};
/// # use dyn_utils::{dyn_object,DynObject};
///
/// // Allows using both `DynObject<dyn Future>` and `DynObject<dyn Future + Send>`
/// #[dyn_object]
/// #[dyn_object(bounds = Send)]
/// trait Callback {
///     fn call(&self, arg: &str);
/// }
///
/// impl<F: Fn(&str)> Callback for F {
///     fn call(&self, arg: &str) {
///         self(arg)
///     }
/// }
///
/// // no allocation
/// let callback = DynObject::<dyn Callback>::new(|arg: &str| println!("{arg}"));
/// ```
///
/// # Limitations
///
/// When combined to [`dyn_trait`], generic parameters are not supported.
///
/// ```compile_fail
/// #[dyn_utils::dyn_trait(trait = DynCallback)]
/// #[dyn_trait(dyn_utils::dyn_object)]
/// trait Callback<T> {
///     fn call(&self, arg: T) -> impl Future<Output = ()> + Send;
/// }
/// ```
///
/// When possible, using associated types instead overcomes this limitation
///
/// ```rust
/// #[dyn_utils::dyn_trait(trait = DynCallback)]
/// #[dyn_trait(dyn_utils::dyn_object)]
/// trait Callback {
///     type Arg;
///     fn call(&self, arg: Self::Arg) -> impl Future<Output = ()> + Send;
/// }
/// ```
pub use dyn_utils_macros::dyn_object;
/// Generate a dyn compatible trait from a given trait declaration.
///
/// Method with a return-position impl trait, such as async method, are converted to return
/// a `DynObject`. Other non dyn-compatible items are filtered.
///
/// # Arguments
///
/// - `trait`: The generated dyn-compatible trait identifier, or a string template; default to
///   `"Dyn{}".
/// - `remote`: Path to the concrete trait used in the implementation; the trait declaration must
///   be pasted. It allows supporting traits defined in other crates.
///
/// # Trait attributes
///
/// Any `#[dyn_trait(...)]` attribute is converted to `#[...]` attribute and applied to the
/// generated dyn-compatible trait. It can be used to apply [`dyn_object`](attr.dyn_object.html)
/// to the generated trait.
///
/// # Method attributes
///
/// Methods with are return-position impl trait, such as async methods, can be decorated with
/// `#[dyn_trait(...)]` attribute with the following arguments:
///
/// - `try_sync`: (must be applied to method returning `Future`) Generates an additional method
///   suffixed with `_try_sync`, with a optimized execution path when the concrete method is
///   synchronous and decorated with [`sync`](attr.sync.html).
/// - `storage`: Defines the default storage in the returned `DynObject`. Each method adds a
///   generic storage parameter, whose default value is `dyn_utils::DefaultStorage` when not
///   specified with the argument.
///
/// # Examples
///
/// ```rust
/// #[dyn_utils::dyn_trait(trait = DynCallback)] // make the trait dyn-compatible
/// #[dyn_trait(dyn_utils::dyn_object)] // make the dyn-compatible trait usable with DynObject
/// trait Callback {
///     #[dyn_trait(try_sync)] // add `call_try_sync` method with synchronous shortcut
///     #[dyn_trait(storage = dyn_utils::storage::Raw<128>)] // use `Raw<128> as default storage
///     fn call(&self, arg: &str) -> impl Future<Output = ()> + Send;
/// }
/// ```
pub use dyn_utils_macros::dyn_trait;
/// Mark an async method as internally synchronous.
///
/// The trait declaration must have been decorated with [`dyn_trait`],
/// and the trait method declaration with [`try_sync`](dyn_trait#method-attributes).
///
/// # Examples
///
/// ```rust
/// #[dyn_utils::dyn_trait]
/// trait Callback {
///     #[dyn_trait(try_sync)]
///     fn call(&self, arg: &str) -> impl Future<Output = ()> + Send;
/// }
///
/// struct HelloCallback;
/// impl Callback for HelloCallback {
///     #[dyn_utils::sync]
///     async fn call(&self, arg: &str) {
///         println!("Hello {arg}!");
///     }
/// }
/// ```
pub use dyn_utils_macros::sync;
