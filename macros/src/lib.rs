#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ImplItemFn, ItemTrait, Token, parse_macro_input, parse_quote, punctuated::Punctuated};

use crate::{macros::bail, methods::is_sync_constant};

mod dyn_storage;
mod dyn_trait;
mod macros;
mod methods;
mod utils;

#[proc_macro_attribute]
pub fn dyn_trait(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let r#trait = parse_macro_input!(item as ItemTrait);
    let mut remote = None;
    let mut dyn_trait = format_ident!("{}Dyn", r#trait.ident);
    let args_parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("remote") {
            meta.input.parse::<Token![=]>()?;
            remote = Some(meta.input.parse()?);
        } else {
            dyn_trait = meta.path.require_ident()?.clone();
        }
        Ok(())
    });
    parse_macro_input!(args with args_parser);
    dyn_trait::dyn_trait_impl(r#trait, dyn_trait, remote)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn sync(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    sync_impl(parse_macro_input!(item as ImplItemFn))
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn sync_impl(method: ImplItemFn) -> syn::Result<TokenStream> {
    if method.sig.asyncness.is_none() {
        bail!(
            method.sig.fn_token, // Because nightly doesn't give the same span for `method`
            "`dyn_utils::sync` must be used on async method"
        );
    }
    let method_name = &method.sig.ident;
    let mut sync_method = method.clone();
    sync_method.sig.asyncness = None;
    sync_method.sig.ident = format_ident!("{method_name}_sync");
    let is_sync = is_sync_constant(&method.sig, true);
    Ok(quote! {
        #method
        #sync_method
        #is_sync
    })
}

#[proc_macro_attribute]
pub fn dyn_storage(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let r#trait = parse_macro_input!(item as ItemTrait);
    let mut remote = None;
    let mut bounds = Punctuated::new();
    let mut crate_path = parse_quote!(::dyn_utils);
    let args_parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("remote") {
            meta.input.parse::<Token![=]>()?;
            remote = Some(meta.input.parse()?);
        } else if meta.path.is_ident("bounds") {
            meta.input.parse::<Token![=]>()?;
            bounds = Punctuated::parse_terminated(meta.input)?;
        } else if meta.path.is_ident("crate") {
            meta.input.parse::<Token![=]>()?;
            crate_path = meta.input.parse()?;
        } else {
            bail!(meta.path, "unknown attribute");
        }
        Ok(())
    });
    parse_macro_input!(args with args_parser);
    dyn_storage::dyn_storage_impl(r#trait, remote, bounds, crate_path)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
