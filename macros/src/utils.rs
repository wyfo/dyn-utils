use std::collections::HashSet;

use syn::{
    CapturedParam, GenericArgument, GenericParam, Generics, Lifetime, LifetimeParam, PathArguments,
    Receiver, ReturnType, TraitItemFn, Type, TypeImplTrait, TypeParamBound, TypeReference,
    parse_quote, visit_mut::VisitMut,
};

use crate::macros::try_match;

pub(crate) fn return_type(method: &TraitItemFn) -> Option<&Type> {
    try_match!(&method.sig.output, ReturnType::Type(_, ty) => ty.as_ref())
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
