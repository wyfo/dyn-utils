use quote::{format_ident, quote, ToTokens};
use syn::{
    parse_quote as p, visit_mut::VisitMut, FnArg, GenericParam, ImplItemFn, Path, Token, TraitItemFn, Type,
    TypeImplTrait, TypeParamBound, TypeTraitObject, Visibility,
};

use crate::{
    macros::bail,
    utils::{forward_method, is_future, return_type},
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
    pin: bool,
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
        method.sig.output = p!(-> impl Future<Output = #output>);
    }
    method
}

pub(crate) fn parse_method_attr(
    method: &mut TraitItemFn,
    ret: &TypeImplTrait,
) -> syn::Result<MethodAttrs> {
    let mut attrs = MethodAttrs::default();
    let is_future = is_future(ret);
    for attr in (method.attrs).extract_if(.., |attr| attr.path().is_ident("dyn_utils")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("try_sync") {
                if !is_future {
                    bail!(meta.path, "`try_sync` must be used on async function");
                }
                attrs.try_sync = true;
            }
            if meta.path.is_ident("pin") {
                attrs.pin = true;
            }
            if meta.path.is_ident("storage") {
                attrs.storage = Some(meta.input.parse()?);
            }
            Ok(())
        })?;
    }
    attrs.pin |= is_future;
    Ok(attrs)
}

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

pub(crate) fn with_storage_method(method: &TraitItemFn, ret: &TypeImplTrait) -> TraitItemFn {
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
            .chain([p!('__dyn)])
            .collect(),
    };
    let mut method = method.clone();
    method.sig.ident = format_ident!("{}_with_storage", method.sig.ident);
    (method.sig.generics.params.iter_mut())
        .for_each(|param| captured.visit_generic_param_mut(param));
    method.sig.generics.params.push(p!('__dyn));
    method.sig.generics.params.push(p!('__storage));
    (method.sig.inputs.iter_mut()).for_each(|arg| captured.visit_fn_arg_mut(arg));
    let mut storage_type = quote!(&'__storage mut ::core::option::Option<::dyn_utils::storage::DynStorage<::dyn_utils::DefaultStorage, ::dyn_utils::private::DynVTable, #dyn_ret>>);
    let mut storage_dyn_ret = quote!(&'__storage mut (#dyn_ret));
    if is_future(ret) {
        storage_type = quote!(::core::pin::Pin<#storage_type>);
        storage_dyn_ret = quote!(::core::pin::Pin<#storage_dyn_ret>);
    }
    (method.sig.inputs).push(p!(__storage: #storage_type));
    method.sig.output = p!(-> #storage_dyn_ret);
    method
}

pub(crate) fn impl_method(
    method: &TraitItemFn,
    with_storage_method: Option<&TraitItemFn>,
) -> ImplItemFn {
    let forward = forward_method(method);
    let block = if let Some(ws) = with_storage_method {
        let insert = match return_type(ws).unwrap() {
            Type::Path(/* Pin */ _) => quote!(insert_into_storage_pinned),
            Type::Reference(_) => quote!(insert_into_storage),
            _ => unreachable!(),
        };
        p!({ unsafe { ::dyn_utils::private::#insert(#forward, __storage) } })
    } else {
        p!({ #forward })
    };
    ImplItemFn {
        attrs: vec![],
        vis: Visibility::Inherited,
        defaultness: None,
        sig: with_storage_method.unwrap_or(method).sig.clone(),
        block,
    }
}
