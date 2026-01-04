use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::{
    Block, FnArg, GenericArgument, GenericParam, ImplItemFn, PatIdent, Path, PathArguments,
    PathSegment, ReturnType, Signature, TraitItemFn, TraitItemType, Type, TypeImplTrait,
    TypeParamBound, Visibility, visit_mut::VisitMut,
};

use crate::macros::try_match;

pub(crate) trait IteratorExt: Iterator + Sized {
    fn update(self, mut f: impl FnMut(&mut Self::Item)) -> impl Iterator<Item = Self::Item> {
        self.map(move |mut item| {
            f(&mut item);
            item
        })
    }

    fn collect_vec(self) -> Vec<Self::Item> {
        self.collect()
    }
}

impl<I: Iterator> IteratorExt for I {}

pub(crate) fn is_not_generic(ty: &TraitItemType) -> bool {
    ty.generics.params.is_empty() && ty.generics.where_clause.is_none()
}

pub(crate) fn is_dispatchable(method: &TraitItemFn) -> bool {
    let has_dyn_trait_receiver =
        (method.sig.receiver()).is_some_and(|recv| recv.reference.is_some() || is_pinned(&recv.ty));
    let has_no_generic_parameter_except_lifetime =
        (method.sig.generics.params.iter()).all(|p| matches!(p, GenericParam::Lifetime(_)));
    has_dyn_trait_receiver && has_no_generic_parameter_except_lifetime
}

pub(crate) fn return_type(sig: &Signature) -> Option<&Type> {
    try_match!(&sig.output, ReturnType::Type(_, ty) => ty.as_ref())
}

pub(crate) fn last_segments<'a>(path: &'a Path, ident: &str) -> Option<&'a PathSegment> {
    path.segments.last().filter(|s| s.ident == ident)
}

pub(crate) fn is_pinned(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if last_segments(&path.path, "Pin").is_some())
}

pub(crate) fn future_output(ret: &TypeImplTrait) -> Option<&Type> {
    let future = (ret.bounds.iter())
        .filter_map(try_match!(TypeParamBound::Trait))
        .find_map(|bound| last_segments(&bound.path, "Future"))?;
    let args = try_match!(&future.arguments, PathArguments::AngleBracketed)?;
    let output = (args.args.iter())
        .filter_map(try_match!(GenericArgument::AssocType))
        .find(|t| t.ident == "Output")?;
    Some(&output.ty)
}

pub(crate) struct PatternAsArg;

impl VisitMut for PatternAsArg {
    fn visit_pat_ident_mut(&mut self, i: &mut PatIdent) {
        i.by_ref = None;
        i.mutability = None;
        i.subpat = None
    }
}

pub(crate) fn fn_args(sig: &Signature) -> impl Iterator<Item = TokenStream> {
    sig.inputs.iter().map(|arg| match arg {
        FnArg::Receiver(_) => quote!(self),
        FnArg::Typed(arg) => {
            let mut pat = arg.pat.as_ref().clone();
            PatternAsArg.visit_pat_mut(&mut pat);
            pat.to_token_stream()
        }
    })
}

pub(crate) fn impl_method(sig: Signature, block: Block) -> ImplItemFn {
    ImplItemFn {
        attrs: vec![],
        vis: Visibility::Inherited,
        defaultness: None,
        sig,
        block,
    }
}

pub(crate) fn respan(tokens: TokenStream, span: Span) -> TokenStream {
    (tokens.into_iter())
        .update(|mut tt| {
            if let TokenTree::Group(group) = &mut tt {
                *group = Group::new(group.delimiter(), respan(group.stream(), span));
            }
            tt.set_span(span);
        })
        .collect()
}
