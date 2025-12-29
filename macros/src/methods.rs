use quote::quote;
use syn::{
    parse_quote as p, visit_mut::VisitMut, ImplItemFn, Token, TraitItemFn, Type, TypeImplTrait,
    TypeParamBound, TypeTraitObject, Visibility,
};

use crate::{
    utils::{forward_method, future_output, return_type, with_storage_suffix},
    CapturedLifetimes,
};

pub fn to_dyn_method(method: &TraitItemFn) -> TraitItemFn {
    let mut method = TraitItemFn {
        attrs: method.attrs.clone(),
        sig: method.sig.clone(),
        default: None,
        semi_token: None,
    };
    if method.sig.asyncness.is_some() {
        method.sig.asyncness = None;
        let output = return_type(&method);
        method.sig.output = p!(-> impl Future<Output = (#output)>);
    }
    method
}

pub fn with_storage_method(method: &TraitItemFn, ret: &TypeImplTrait) -> TraitItemFn {
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
    let fut_out = future_output(ret);
    let mut method = method.clone();
    method.sig.ident = with_storage_suffix(&method);
    (method.sig.generics.params.iter_mut())
        .for_each(|param| captured.visit_generic_param_mut(param));
    method.sig.generics.params.push(p!('__dyn));
    method.sig.generics.params.push(p!('__storage));
    (method.sig.inputs.iter_mut()).for_each(|arg| captured.visit_fn_arg_mut(arg));
    let mut storage_type = quote!(&'__storage mut ::core::option::Option<::dyn_utils::storage::DynStorage<::dyn_utils::DefaultStorage, ::dyn_utils::private::DynVTable, #dyn_ret>>);
    let mut storage_dyn_ret = quote!(&'__storage mut (#dyn_ret));
    if fut_out.is_some() {
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
        attrs: method.attrs.clone(),
        vis: Visibility::Inherited,
        defaultness: None,
        sig: with_storage_method.unwrap_or(method).sig.clone(),
        block,
    }
}
