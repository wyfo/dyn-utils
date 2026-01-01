use syn::{ReturnType, TraitItemFn, Type, TypeImplTrait, TypeParamBound};

use crate::macros::try_match;

#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) fn is_future(ty: &TypeImplTrait) -> bool {
    (ty.bounds.iter())
        .filter_map(try_match!(TypeParamBound::Trait))
        .any(|bound| bound.path.segments.last().unwrap().ident == "Future")
}

pub(crate) fn return_type(method: &TraitItemFn) -> Option<&Type> {
    try_match!(&method.sig.output, ReturnType::Type(_, ty) => ty.as_ref())
}
