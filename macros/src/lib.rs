#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use proc_macro::TokenStream;
use syn::{Path, meta::ParseNestedMeta, parse::Parse, parse_macro_input, parse_quote};

use crate::{
    dyn_object::dyn_object_impl, dyn_trait::dyn_trait_impl, macros::bail, sync::sync_impl,
};

mod dyn_object;
mod dyn_trait;
mod macros;
mod sync;
mod utils;

fn crate_name() -> Path {
    parse_quote!(::dyn_utils)
}

trait MacroArgs: Default {
    fn parse_meta(&mut self, meta: ParseNestedMeta) -> syn::Result<()>;
}
impl MacroArgs for () {
    fn parse_meta(&mut self, meta: ParseNestedMeta) -> syn::Result<()> {
        bail!(meta.path, "unknown attribute");
    }
}

fn macro_impl<Item: Parse, Args: MacroArgs>(
    impl_fn: fn(Item, Args) -> syn::Result<proc_macro2::TokenStream>,
    item: TokenStream,
    args: TokenStream,
) -> TokenStream {
    let parsed_item = syn::parse_macro_input!(item as Item);
    let mut parsed_args = Args::default();
    let args_parser = syn::meta::parser(|m| parsed_args.parse_meta(m));
    parse_macro_input!( args  with args_parser );
    impl_fn(parsed_item, parsed_args)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn dyn_trait(args: TokenStream, item: TokenStream) -> TokenStream {
    macro_impl(dyn_trait_impl, item, args)
}

#[proc_macro_attribute]
pub fn sync(args: TokenStream, item: TokenStream) -> TokenStream {
    macro_impl(sync_impl, item, args)
}

#[proc_macro_attribute]
pub fn dyn_object(args: TokenStream, item: TokenStream) -> TokenStream {
    macro_impl(dyn_object_impl, item, args)
}
