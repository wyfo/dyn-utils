use std::borrow::Cow;

use proc_macro2::Ident;
use quote::format_ident;
use syn::{
    parse_quote, AssocType, Expr, FnArg, GenericArgument, GenericParam, PathArguments, ReturnType,
    TraitItemFn, Type, TypeImplTrait, TypeParamBound,
};

use crate::macros::try_match;

pub(crate) fn with_storage_suffix(method: &TraitItemFn) -> Ident {
    format_ident!("{}_with_storage", method.sig.ident)
}

pub(crate) fn future_output(ty: &TypeImplTrait) -> Option<&AssocType> {
    let future = (ty.bounds.iter())
        .filter_map(try_match!(TypeParamBound::Trait))
        .find_map(|bound| bound.path.segments.last().filter(|s| s.ident == "Future"))?;
    let args = try_match!(&future.arguments, PathArguments::AngleBracketed)?;
    (args.args.iter())
        .filter_map(try_match!(GenericArgument::AssocType))
        .find(|t| t.ident == "Output")
}

pub(crate) fn return_type(method: &TraitItemFn) -> Option<&Type> {
    try_match!(&method.sig.output, ReturnType::Type(_, ty) => ty.as_ref())
}

pub(crate) fn forward_method(method: &TraitItemFn) -> Expr {
    let method_name = &method.sig.ident;
    let generics = method
        .sig
        .generics
        .params
        .iter()
        .filter(|p| !matches!(p, &GenericParam::Lifetime(_)));
    let args = method.sig.inputs.iter().map(|arg| match arg {
        FnArg::Receiver(_) => Cow::Owned(parse_quote!(self)),
        FnArg::Typed(arg) => Cow::Borrowed(&arg.pat),
    });
    parse_quote!(__Dyn::#method_name::<#(#generics,)*>(#(#args,)*))
}
