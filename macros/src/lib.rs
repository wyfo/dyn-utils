#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    GenericParam, ImplItem, ImplItemFn, ItemTrait, TraitItem, TraitItemFn, Type, parse_macro_input,
    parse_quote, parse_quote_spanned, spanned::Spanned,
};

use crate::{
    macros::{bail, try_match},
    methods::{
        dyn_method, handle_async_method, impl_method, is_dyn_compatible, is_sync_constant,
        parse_method_attrs, sync_method, try_sync_dyn_method, try_sync_impl_method,
    },
    utils::return_type,
};

mod macros;
mod methods;
mod utils;

#[proc_macro_attribute]
pub fn dyn_compatible(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    dyn_compatible_impl(parse_macro_input!(item as ItemTrait))
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn dyn_compatible_impl(mut r#trait: ItemTrait) -> syn::Result<TokenStream> {
    let trait_name = &r#trait.ident;
    let mut dyn_items = Vec::new();
    let mut storages = Vec::<GenericParam>::new();
    let mut impl_items = Vec::<ImplItem>::new();
    let mut additional_trait_items = Vec::new();
    for item in r#trait.items.iter_mut() {
        match item {
            TraitItem::Type(ty) => {
                let (impl_generics, ty_generics, where_clause) = ty.generics.split_for_impl();
                let ty_name = &ty.ident;
                impl_items.push(parse_quote!(type #ty_name #impl_generics = <__Dyn as #trait_name>::#ty_name #ty_generics #where_clause;));
                dyn_items.push(item.clone());
            }
            TraitItem::Fn(method) if is_dyn_compatible(method) => {
                let attrs = parse_method_attrs(method)?;
                let mut method = TraitItemFn {
                    attrs: method.attrs.clone(),
                    sig: method.sig.clone(),
                    default: None,
                    semi_token: None,
                };
                handle_async_method(&mut method);
                if let Some(ret) = return_type(&method).and_then(try_match!(Type::ImplTrait)) {
                    let storage =
                        format_ident!("__Storage{}", method.sig.ident.to_string().to_pascal_case());
                    let default_storage = attrs.storage();
                    storages.push(parse_quote_spanned!(default_storage.span() => #storage: ::dyn_utils::Storage = #default_storage));
                    let dyn_method = dyn_method(&method, ret, &storage);
                    let impl_method = impl_method(&method, Some(&dyn_method));
                    if attrs.try_sync() {
                        additional_trait_items.push(sync_method(&method, ret)?.into());
                        let is_sync = is_sync_constant(&method.sig, false);
                        additional_trait_items.push(parse_quote!(#[doc(hidden)] #is_sync));
                        dyn_items.push(try_sync_dyn_method(&dyn_method).into());
                        impl_items.push(try_sync_impl_method(&impl_method).into());
                    }
                    dyn_items.push(dyn_method.into());
                    impl_items.push(impl_method.into());
                } else {
                    attrs.check_no_attr()?;
                    impl_items.push(impl_method(&method, None).into());
                    dyn_items.push(method.into());
                }
            }
            _ => {}
        }
    }
    r#trait.items.extend(additional_trait_items);
    let dyn_trait = format_ident!("Dyn{}", r#trait.ident);
    let unsafety = &r#trait.unsafety;
    let vis = &r#trait.vis;
    let supertraits = &r#trait.supertraits;
    let (_, trait_generics_ty, where_clause) = r#trait.generics.split_for_impl();
    let mut dyn_generics = r#trait.generics.clone();
    dyn_generics.params.extend(storages);
    let (_, dyn_generics_ty, _) = dyn_generics.split_for_impl();
    let mut impl_generics = dyn_generics.clone();
    impl_generics
        .params
        .push(parse_quote!(__Dyn: #trait_name #trait_generics_ty));
    let (impl_generics, _, _) = impl_generics.split_for_impl();
    Ok(quote! {
        #r#trait
        #vis #unsafety trait #dyn_trait #dyn_generics #supertraits #where_clause { #(#dyn_items)* }
        #unsafety impl #impl_generics #dyn_trait #dyn_generics_ty for __Dyn #where_clause { #(#impl_items)* }
    })
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
