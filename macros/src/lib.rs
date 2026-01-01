#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    CapturedParam, GenericParam, Generics, ItemImpl, ItemTrait, Lifetime, LifetimeParam, Receiver,
    TraitItem, Type, TypeImplTrait, TypeParamBound, TypeReference, parse_macro_input, parse_quote,
    visit_mut::VisitMut,
};

use crate::{
    macros::try_match,
    methods::{dyn_method, impl_method, is_dyn_compatible, to_dyn_method},
    utils::return_type,
};

mod macros;
mod methods;
mod utils;

#[cfg_attr(coverage_nightly, coverage(off))]
#[proc_macro_attribute]
pub fn dyn_compatible(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    dyn_compatible_impl(&parse_macro_input!(item as ItemTrait))
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn dyn_compatible_impl(r#trait: &ItemTrait) -> syn::Result<TokenStream> {
    let trait_name = &r#trait.ident;
    let mut with_storage_items = Vec::new();
    let mut impl_items = Vec::new();
    for item in r#trait.items.iter() {
        match item {
            TraitItem::Type(ty) => {
                let (impl_generics, ty_generics, where_clause) = ty.generics.split_for_impl();
                let ty_name = &ty.ident;
                impl_items.push(parse_quote!(type #ty_name #impl_generics = <__Dyn as #trait_name>::#ty_name #ty_generics #where_clause;));
                with_storage_items.push(item.clone());
            }
            TraitItem::Fn(method) if is_dyn_compatible(method) => {
                let method = to_dyn_method(method);
                if let Some(ret) = return_type(&method).and_then(try_match!(Type::ImplTrait)) {
                    let with_storage = dyn_method(&method, ret);
                    impl_items.push(impl_method(&method, Some(&with_storage)).into());
                    with_storage_items.push(with_storage.into());
                } else {
                    impl_items.push(impl_method(&method, None).into());
                    with_storage_items.push(method.into());
                }
            }
            _ => {}
        }
    }
    let dyn_trait = ItemTrait {
        attrs: vec![],
        vis: r#trait.vis.clone(),
        unsafety: r#trait.unsafety,
        auto_token: r#trait.auto_token,
        restriction: r#trait.restriction.clone(),
        trait_token: r#trait.trait_token,
        ident: format_ident!("Dyn{}", r#trait.ident),
        generics: r#trait.generics.clone(),
        colon_token: r#trait.colon_token,
        supertraits: r#trait.supertraits.clone(),
        brace_token: r#trait.brace_token,
        items: with_storage_items,
    };
    let (_, ty_generics, _) = r#trait.generics.split_for_impl();
    let mut impl_generics = r#trait.generics.clone();
    impl_generics
        .params
        .push(parse_quote!(__Dyn: #trait_name #ty_generics));
    let impl_with_storage = ItemImpl {
        attrs: vec![],
        defaultness: None,
        unsafety: r#trait.unsafety,
        impl_token: Default::default(),
        generics: impl_generics,
        trait_: Some((None, dyn_trait.ident.clone().into(), parse_quote!(for))),
        self_ty: parse_quote!(__Dyn),
        brace_token: Default::default(),
        items: impl_items,
    };
    Ok(quote! {
        #r#trait
        #dyn_trait
        #impl_with_storage
    })
}

struct CapturedLifetimes {
    dyn_lt: Lifetime,
    default_lt: Lifetime,
    captured: HashSet<Lifetime>,
}

impl CapturedLifetimes {
    fn new(ret: &TypeImplTrait, generics: &Generics) -> Self {
        Self {
            dyn_lt: parse_quote!('__dyn),
            default_lt: parse_quote!('_),
            captured: match (ret.bounds.iter()).find_map(try_match!(TypeParamBound::PreciseCapture))
            {
                Some(c) => (c.params.iter())
                    .filter_map(try_match!(CapturedParam::Lifetime(l) => l.clone()))
                    .collect(),
                None => (generics.params.iter())
                    .filter_map(try_match!(GenericParam::Lifetime(l) => l.lifetime.clone()))
                    .chain([parse_quote!('_)])
                    .collect(),
            },
        }
    }
}

impl VisitMut for CapturedLifetimes {
    fn visit_lifetime_mut(&mut self, i: &mut Lifetime) {
        if *i == self.default_lt && self.captured.contains(i) {
            *i = self.dyn_lt.clone();
        }
    }

    fn visit_lifetime_param_mut(&mut self, i: &mut LifetimeParam) {
        if self.captured.contains(&i.lifetime) {
            i.bounds.push(self.dyn_lt.clone());
        }
    }

    fn visit_receiver_mut(&mut self, i: &mut Receiver) {
        if let Some((_, lt @ None)) = &mut i.reference {
            *lt = Some(self.default_lt.clone());
        }
        syn::visit_mut::visit_receiver_mut(self, i);
    }

    fn visit_type_reference_mut(&mut self, i: &mut TypeReference) {
        if i.lifetime.is_none() {
            i.lifetime = Some(self.default_lt.clone());
        }
        syn::visit_mut::visit_type_reference_mut(self, i);
    }
}
