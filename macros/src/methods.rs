use std::borrow::Cow;

use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    FnArg, GenericParam, ImplItemFn, Path, ReturnType, Signature, Token, TraitItemFn,
    TypeImplTrait, TypeParamBound, TypeTraitObject, Visibility, parse_quote,
    punctuated::Punctuated, visit_mut::VisitMut,
};

use crate::{
    macros::{bail, try_match},
    utils::{CapturedLifetimes, future_output, is_pinned, return_type},
};

pub(crate) fn is_dyn_compatible(method: &TraitItemFn) -> bool {
    let has_dyn_compatible_receiver =
        (method.sig.receiver()).is_some_and(|recv| recv.reference.is_some() || is_pinned(&recv.ty));
    let has_no_generic_parameter_except_lifetime =
        (method.sig.generics.params.iter()).all(|p| matches!(p, GenericParam::Lifetime(_)));
    has_dyn_compatible_receiver && has_no_generic_parameter_except_lifetime
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

    pub(crate) fn try_sync(&self) -> bool {
        self.try_sync.is_some()
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
    method.sig.output = parse_quote!(-> ::dyn_utils::DynStorage<#dyn_ret, #storage>);
    method
}

fn forward_call(method: &Ident, args: &Punctuated<FnArg, Token![,]>) -> TokenStream {
    let args = args.iter().map(|arg| match arg {
        FnArg::Receiver(_) => Cow::Owned(parse_quote!(self)),
        FnArg::Typed(arg) => Cow::Borrowed(&arg.pat),
    });
    quote!(__Dyn::#method(#(#args,)*))
}

pub(crate) fn impl_method(dyn_method: &TraitItemFn, dyn_storage: bool) -> ImplItemFn {
    let call = forward_call(&dyn_method.sig.ident, &dyn_method.sig.inputs);
    ImplItemFn {
        attrs: vec![],
        vis: Visibility::Inherited,
        defaultness: None,
        sig: dyn_method.sig.clone(),
        block: if dyn_storage {
            parse_quote!({ ::dyn_utils::DynStorage::new(#call) })
        } else {
            parse_quote!({ #call })
        },
    }
}

pub(crate) fn sync_method(method: &TraitItemFn, ret: &TypeImplTrait) -> syn::Result<TraitItemFn> {
    let Some(output) = future_output(ret) else {
        bail!(
            method.sig.fn_token, // Because nightly doesn't give the same span for `method`
            "`try_sync` must be used on async methods"
        );
    };
    let mut sync_method = method.clone();
    sync_method.attrs.push(parse_quote!(#[doc(hidden)]));
    sync_method
        .attrs
        .push(parse_quote!(#[allow(unused_variables)]));
    sync_method.sig.output = parse_quote!(-> #output);
    let method_name = &method.sig.ident;
    sync_method.sig.ident = format_ident!("{method_name}_sync");
    sync_method.default = Some(parse_quote!({ ::core::unimplemented!() }));
    Ok(sync_method)
}

pub(crate) fn is_sync_constant(signature: &Signature, value: bool) -> TokenStream {
    let is_sync = format_ident!("{}_IS_SYNC", signature.ident.to_string().to_uppercase());
    quote!(const #is_sync: bool = #value;)
}

fn try_sync_signature(signature: &Signature) -> Signature {
    let mut signature = signature.clone();
    let ident = &signature.ident;
    signature.ident = format_ident!("{ident}_try_sync");
    let output = try_match!(&signature.output, ReturnType::Type(_, ty) => ty.as_ref()).unwrap();
    signature.output = parse_quote!(-> ::dyn_utils::TrySync<#output>);
    signature
}

pub(crate) fn try_sync_dyn_method(dyn_method: &TraitItemFn) -> TraitItemFn {
    let mut method = dyn_method.clone();
    method.sig = try_sync_signature(&method.sig);
    method
}

pub(crate) fn try_sync_impl_method(impl_method: &ImplItemFn) -> ImplItemFn {
    let method_name = &impl_method.sig.ident;
    let mut method = impl_method.clone();
    method.sig = try_sync_signature(&method.sig);
    let is_sync = format_ident!("{}_IS_SYNC", method_name.to_string().to_uppercase());
    let sync_call = forward_call(&format_ident!("{}_sync", method_name), &method.sig.inputs);
    let async_call = forward_call(method_name, &method.sig.inputs);
    method.block = parse_quote!({
       if __Dyn::#is_sync {
            ::dyn_utils::TrySync::Sync(#sync_call)
        } else {
            ::dyn_utils::TrySync::Async(::dyn_utils::DynStorage::new(#async_call))
        }
    });
    method
}

// for coverage
#[cfg(test)]
mod tests {
    use syn::{TraitItemFn, parse_quote};

    use crate::methods::try_sync_signature;

    #[test]
    fn try_sync_signature_ok() {
        let func: TraitItemFn = parse_quote! {
            fn method(&self) -> ::dyn_utils::DynStorage<dyn Future<Output=()>, __StorageMethod>;
        };
        try_sync_signature(&func.sig);
    }

    #[test]
    #[should_panic]
    fn try_sync_signature_unreachable() {
        let func: TraitItemFn = parse_quote! {
            fn method(&self);
        };
        try_sync_signature(&func.sig);
    }
}
