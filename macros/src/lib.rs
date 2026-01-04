#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use syn::{ImplItemFn, ItemTrait, parse_macro_input};

use crate::{
    dyn_storage::{DynStorageOpts, dyn_storage_impl},
    dyn_trait::{DynTraitOpts, dyn_trait_impl},
    macros::macro_impl,
    sync::sync_impl,
};

mod dyn_storage;
mod dyn_trait;
mod macros;
mod sync;
mod utils;

#[proc_macro_attribute]
pub fn dyn_trait(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    macro_impl!(dyn_trait_impl, item as ItemTrait, args as DynTraitOpts)
}

#[proc_macro_attribute]
pub fn sync(
    _args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    macro_impl!(sync_impl, item as ImplItemFn)
}

#[proc_macro_attribute]
pub fn dyn_storage(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    macro_impl!(dyn_storage_impl, item as ItemTrait, args as DynStorageOpts)
}
