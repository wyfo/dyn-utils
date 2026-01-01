use std::borrow::Cow;

use syn::{
    parse_quote, Expr, FnArg, GenericParam, ReturnType,
    TraitItemFn, Type, TypeImplTrait, TypeParamBound,
};

use crate::macros::try_match;

pub(crate) fn is_future(ty: &TypeImplTrait) -> bool {
    (ty.bounds.iter())
        .filter_map(try_match!(TypeParamBound::Trait))
        .any(|bound| bound.path.segments.last().unwrap().ident == "Future")
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
