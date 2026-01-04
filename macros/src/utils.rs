use std::collections::HashSet;

use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::{
    Block, CapturedParam, FnArg, GenericArgument, GenericParam, Generics, ImplItemFn, Lifetime,
    LifetimeParam, PatIdent, PathArguments, Receiver, ReturnType, Signature, TraitItemFn, Type,
    TypeImplTrait, TypeParamBound, TypeReference, Visibility, parse_quote, visit_mut::VisitMut,
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

pub(crate) fn return_type(method: &TraitItemFn) -> Option<&Type> {
    try_match!(&method.sig.output, ReturnType::Type(_, ty) => ty.as_ref())
}

pub(crate) fn is_pinned(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().unwrap().ident == "Pin")
}

pub(crate) fn future_output(ret: &TypeImplTrait) -> Option<&Type> {
    let future = (ret.bounds.iter())
        .filter_map(try_match!(TypeParamBound::Trait))
        .find_map(|bound| bound.path.segments.last().filter(|s| s.ident == "Future"))?;
    let args = try_match!(&future.arguments, PathArguments::AngleBracketed)?;
    let output = (args.args.iter())
        .filter_map(try_match!(GenericArgument::AssocType))
        .find(|t| t.ident == "Output")?;
    Some(&output.ty)
}

pub(crate) fn to_impl_method(method: &TraitItemFn, block: Block) -> ImplItemFn {
    ImplItemFn {
        attrs: vec![],
        vis: Visibility::Inherited,
        defaultness: None,
        sig: method.sig.clone(),
        block,
    }
}

pub struct CapturedLifetimes {
    dyn_lt: Lifetime,
    default_lt: Lifetime,
    captured: HashSet<Lifetime>,
}

impl CapturedLifetimes {
    pub(crate) fn new(ret: &TypeImplTrait, generics: &Generics) -> Self {
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

pub(crate) struct PatternAsArg;

impl VisitMut for PatternAsArg {
    fn visit_pat_ident_mut(&mut self, i: &mut PatIdent) {
        i.by_ref = None;
        i.mutability = None;
        i.subpat = None
    }
}

pub(crate) fn fn_args(signature: &Signature) -> impl Iterator<Item = TokenStream> {
    signature.inputs.iter().map(|arg| match arg {
        FnArg::Receiver(_) => quote!(self),
        FnArg::Typed(arg) => {
            let mut pat = arg.pat.as_ref().clone();
            PatternAsArg.visit_pat_mut(&mut pat);
            pat.to_token_stream()
        }
    })
}
