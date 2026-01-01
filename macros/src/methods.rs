use std::borrow::Cow;

use quote::{quote, ToTokens};
use syn::{
    parse_quote, visit_mut::VisitMut, FnArg, GenericParam, ImplItemFn, Path, Token, TraitItemFn,
    TypeImplTrait, TypeParamBound, TypeTraitObject, Visibility,
};

use crate::{
    macros::bail,
    utils::{is_future, return_type},
    CapturedLifetimes,
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

#[derive(Default)]
pub(crate) struct MethodAttrs {
    try_sync: bool,
    storage: Option<Path>,
}

pub(crate) fn to_dyn_method(method: &TraitItemFn) -> TraitItemFn {
    let mut method = TraitItemFn {
        attrs: method.attrs.clone(),
        sig: method.sig.clone(),
        default: None,
        semi_token: None,
    };
    if method.sig.asyncness.is_some() {
        method.sig.asyncness = None;
        let output = return_type(&method).map_or_else(|| quote!(()), ToTokens::to_token_stream);
        method.sig.output = parse_quote!(-> impl Future<Output = #output>);
    }
    method
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[expect(dead_code)]
pub(crate) fn parse_method_attr(
    method: &mut TraitItemFn,
    ret: &TypeImplTrait,
) -> syn::Result<MethodAttrs> {
    let mut attrs = MethodAttrs::default();
    for attr in (method.attrs).extract_if(.., |attr| attr.path().is_ident("dyn_utils")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("try_sync") {
                if !is_future(ret) {
                    bail!(meta.path, "`try_sync` must be used on async function");
                }
                attrs.try_sync = true;
            }
            if meta.path.is_ident("storage") {
                attrs.storage = Some(meta.input.parse()?);
            }
            Ok(())
        })?;
    }
    Ok(attrs)
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[expect(dead_code)]
pub(crate) fn check_no_method_attr(method: &TraitItemFn) -> syn::Result<()> {
    for attr in method.attrs.iter() {
        if attr.path().is_ident("dyn_utils") {
            bail!(
                attr,
                "`dyn_utils` attribute must be used on a method with Return Position Impl Trait"
            );
        }
    }
    Ok(())
}

pub(crate) fn dyn_method(method: &TraitItemFn, ret: &TypeImplTrait) -> TraitItemFn {
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
    method.sig.output = parse_quote!(-> ::dyn_utils::storage::DynStorage<#dyn_ret>);
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
