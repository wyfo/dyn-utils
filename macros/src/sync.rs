use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{ImplItemFn, Signature};

use crate::macros::bail_method;

pub(super) fn sync_impl(method: ImplItemFn) -> syn::Result<TokenStream> {
    if method.sig.asyncness.is_none() {
        bail_method!(method, "`dyn_utils::sync` must be used on async method");
    }
    let mut sync_method = method.clone();
    sync_method.sig.asyncness = None;
    sync_method.sig.ident = sync_fn(&method.sig);
    let is_sync = is_sync_const(&method.sig);
    Ok(quote! {
        #method
        #sync_method
        const #is_sync: bool = true;
    })
}

pub(crate) fn try_sync_fn(sig: &Signature) -> Ident {
    format_ident!("{}_try_sync", sig.ident)
}

pub(crate) fn sync_fn(sig: &Signature) -> Ident {
    format_ident!("{}_sync", sig.ident)
}

pub(crate) fn is_sync_const(sig: &Signature) -> Ident {
    format_ident!("{}_IS_SYNC", sig.ident.to_string().to_uppercase())
}
