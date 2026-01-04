use heck::ToPascalCase;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{
    GenericParam, ImplItem, ItemTrait, Path, TraitItem, TraitItemFn, Type, parse_quote,
    parse_quote_spanned, spanned::Spanned, visit_mut::VisitMut,
};

use crate::{
    macros::try_match,
    methods::{
        dyn_method, handle_async_method, impl_method, is_dispatchable, is_sync_constant,
        parse_method_attrs, sync_method, try_sync_dyn_method, try_sync_impl_method,
    },
    utils::{PatternAsArg, return_type},
};

pub fn dyn_trait_impl(
    mut r#trait: ItemTrait,
    dyn_trait: Ident,
    remote: Option<Path>,
) -> syn::Result<TokenStream> {
    let include_trait = remote.is_none();
    let dyn_trait_attrs = extract_dyn_trait_attrs(&mut r#trait)?;
    let remote = remote.unwrap_or_else(|| r#trait.ident.clone().into());
    let mut dyn_items = Vec::new();
    let mut storages = Vec::<GenericParam>::new();
    let mut impl_items = Vec::<ImplItem>::new();
    let mut additional_trait_items = Vec::new();
    for item in r#trait.items.iter_mut() {
        match item {
            TraitItem::Type(ty) if ty.generics.params.is_empty() => {
                let (impl_generics, ty_generics, where_clause) = ty.generics.split_for_impl();
                let ty_name = &ty.ident;
                impl_items.push(parse_quote!(type #ty_name #impl_generics = <__Dyn as #remote>::#ty_name #ty_generics #where_clause;));
                dyn_items.push(item.clone());
            }
            TraitItem::Fn(method) if is_dispatchable(method) => {
                let attrs = parse_method_attrs(method)?;
                let mut method = TraitItemFn {
                    attrs: method.attrs.clone(),
                    sig: method.sig.clone(),
                    default: None,
                    semi_token: None,
                };
                PatternAsArg.visit_signature_mut(&mut method.sig);
                handle_async_method(&mut method)?;
                if let Some(ret) = return_type(&method).and_then(try_match!(Type::ImplTrait)) {
                    let storage =
                        format_ident!("__Storage{}", method.sig.ident.to_string().to_pascal_case());
                    let default_storage = attrs.storage();
                    storages.push(parse_quote_spanned!(default_storage.span() => #storage: ::dyn_utils::storage::Storage = #default_storage));
                    let dyn_method = dyn_method(&method, ret, &storage);
                    let impl_method = impl_method(&dyn_method, true);
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
                    impl_items.push(impl_method(&method, false).into());
                    dyn_items.push(method.into());
                }
            }
            _ => {}
        }
    }
    r#trait.items.extend(additional_trait_items);
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
        .push(parse_quote!(__Dyn: #remote #trait_generics_ty));
    let (impl_generics, _, _) = impl_generics.split_for_impl();
    let trait_name = &r#trait.ident;
    let r#trait = include_trait.then_some(&r#trait);
    Ok(quote! {
        #r#trait
        /// Dyn-compatible implementation of
        #[doc = ::core::concat!("[`", stringify!(#trait_name), "`](", stringify!(#remote), ").")]
        #(#dyn_trait_attrs)*
        #vis #unsafety trait #dyn_trait #dyn_generics #supertraits #where_clause { #(#dyn_items)* }
        #unsafety impl #impl_generics #dyn_trait #dyn_generics_ty for __Dyn #where_clause { #(#impl_items)* }
    })
}

fn extract_dyn_trait_attrs(r#trait: &mut ItemTrait) -> syn::Result<Vec<TokenStream>> {
    (r#trait.attrs)
        .extract_if(.., |attr| attr.path().is_ident("dyn_trait"))
        .map(|attr| {
            let meta = &attr.meta.require_list()?.tokens;
            Ok(quote!(#[#meta]))
        })
        .collect()
}
