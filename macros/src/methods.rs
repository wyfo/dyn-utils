use std::borrow::Cow;

use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, quote};
use syn::{
    FnArg, GenericParam, ImplItemFn, Path, Token, TraitItemFn, TypeImplTrait, TypeParamBound,
    TypeTraitObject, Visibility, parse_quote, visit_mut::VisitMut,
};

use crate::{
    macros::bail,
    utils::{CapturedLifetimes, return_type},
};

pub(crate) fn is_dyn_compatible(method: &TraitItemFn) -> bool {
    // `self` receiver requires `Self: Sized` bound, so methods can still be kept
    // (and it's possible to implement them for `Box<dyn Trait>` or `DynStorage`)
    let has_receiver =
        (method.sig.inputs.first()).is_some_and(|arg| matches!(arg, FnArg::Receiver(_)));
    let has_no_generic_parameter_except_lifetime =
        (method.sig.generics.params.iter()).all(|p| matches!(p, GenericParam::Lifetime(_)));
    has_receiver && has_no_generic_parameter_except_lifetime
}

pub(crate) fn handle_async_method(method: &mut TraitItemFn) {
    if method.sig.asyncness.is_some() {
        method.sig.asyncness = None;
        let output = return_type(method).map_or_else(|| quote!(()), ToTokens::to_token_stream);
        method.sig.output = parse_quote!(-> impl Future<Output = #output>);
    }
}

#[derive(Default)]
pub(crate) struct MethodAttrs {
    try_sync: Option<Path>,
    storage: Option<(Path, Path)>,
}

impl MethodAttrs {
    pub fn check_no_attr(&self) -> syn::Result<()> {
        let err = "attribute must be used on a method with Return Position Impl Trait";
        if let Some(attr) = &self.try_sync {
            bail!(attr, err);
        }
        if let Some((attr, _)) = &self.storage {
            bail!(attr, err);
        }
        Ok(())
    }

    pub(crate) fn storage(&self) -> TokenStream {
        match &self.storage {
            Some((_, storage)) => quote!(#storage),
            None => quote!(::dyn_utils::DefaultStorage),
        }
    }
}

pub(crate) fn parse_method_attrs(method: &mut TraitItemFn) -> syn::Result<MethodAttrs> {
    let mut attrs = MethodAttrs::default();
    for attr in (method.attrs).extract_if(.., |attr| attr.path().is_ident("dyn_utils")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("try_sync") {
                attrs.try_sync = Some(meta.path.clone());
            } else if meta.path.is_ident("storage") {
                meta.input.parse::<Token![=]>()?;
                attrs.storage = Some((meta.path, meta.input.parse()?));
            } else {
                bail!(meta.path, "unknown attribute");
            }
            Ok(())
        })?;
    }
    Ok(attrs)
}

pub(crate) fn dyn_method(
    method: &TraitItemFn,
    ret: &TypeImplTrait,
    storage: &Ident,
) -> TraitItemFn {
    let mut captured = CapturedLifetimes::new(ret, &method.sig.generics);
    let dyn_ret = TypeTraitObject {
        dyn_token: Some(Token![dyn](ret.impl_token.span)),
        bounds: (ret.bounds.iter())
            .filter(|b| matches!(b, TypeParamBound::Trait(_)))
            .cloned()
            .map(|mut bound| {
                captured.visit_type_param_bound_mut(&mut bound);
                bound
            })
            .chain([parse_quote!('__dyn)])
            .collect(),
    };
    let mut method = method.clone();
    (method.sig.generics.params.iter_mut())
        .for_each(|param| captured.visit_generic_param_mut(param));
    method.sig.generics.params.push(parse_quote!('__dyn));
    (method.sig.inputs.iter_mut()).for_each(|arg| captured.visit_fn_arg_mut(arg));
    method.sig.output = parse_quote!(-> ::dyn_utils::storage::DynStorage<#dyn_ret, #storage>);
    method
}

pub(crate) fn impl_method(method: &TraitItemFn, dyn_method: Option<&TraitItemFn>) -> ImplItemFn {
    let method_name = &method.sig.ident;
    let args = method.sig.inputs.iter().map(|arg| match arg {
        FnArg::Receiver(_) => Cow::Owned(parse_quote!(self)),
        FnArg::Typed(arg) => Cow::Borrowed(&arg.pat),
    });
    let forward = quote!(__Dyn::#method_name(#(#args,)*));
    ImplItemFn {
        attrs: vec![],
        vis: Visibility::Inherited,
        defaultness: None,
        sig: dyn_method.unwrap_or(method).sig.clone(),
        block: match dyn_method {
            Some(_) => parse_quote!({ ::dyn_utils::DynStorage::new(#forward) }),
            None => parse_quote!({ #forward }),
        },
    }
}
