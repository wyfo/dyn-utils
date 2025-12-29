use std::collections::HashSet;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse_macro_input, parse_quote, visit_mut::VisitMut, CapturedParam, FnArg, GenericArgument, GenericParam,
    Generics, ItemTrait, Lifetime, LifetimeParam, PathArguments, Receiver, ReturnType, Signature,
    Token, TraitItem, TraitItemFn, Type, TypeImplTrait, TypeParamBound,
    TypeReference, TypeTraitObject,
};

macro_rules! try_match {
    ($pattern:pat $(if $guard:expr)? => $result:expr) => {
        |__arg| try_match!(__arg, $pattern $(if $guard)? => $result)
    };
    ($expression:expr, $pattern:pat $(if $guard:expr)? => $result:expr) => {
        match $expression {
            $pattern $(if $guard)? => Some($result),
            _ => None
        }
    };
}

macro_rules! bail {
    ($tokens:expr, $err:expr) => {
        return Err(syn::Error::new(syn::spanned::Spanned::span(&$tokens), $err))
    };
}

#[proc_macro_attribute]
pub fn with_storage(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    with_storage_impl(&parse_macro_input!(item as ItemTrait))
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn with_storage_impl(r#trait: &ItemTrait) -> syn::Result<TokenStream> {
    let mut with_storage_items = Vec::new();
    for item in r#trait.items.iter().cloned() {
        match item {
            TraitItem::Macro(_) => bail!(item, "macro invocations are not supported"),
            TraitItem::Const(_) => {}
            TraitItem::Fn(mut method) => {
                method.default = None;
                if (method.sig.generics.params.iter())
                    .any(|p| !matches!(p, GenericParam::Lifetime(_)))
                {
                    add_sized_bound(&mut method);
                }
                if method.sig.asyncness.is_some() {
                    method.sig.asyncness = None;
                    let output = try_match!(&method.sig.output, ReturnType::Type(_, ty) => ty);
                    method.sig.output = parse_quote!(-> impl Future<Output = (#output)>);
                }
                match impl_return(&method) {
                    Some(ret) => plop(&mut with_storage_items, &method, ret),
                    None => with_storage_items.push(method.into()),
                }
            }
            _ => with_storage_items.push(item),
        }
    }
    let with_storage_trait = ItemTrait {
        attrs: vec![],
        vis: r#trait.vis.clone(),
        unsafety: r#trait.unsafety,
        auto_token: r#trait.auto_token,
        restriction: r#trait.restriction.clone(),
        trait_token: r#trait.trait_token,
        ident: format_ident!("{}WithStorage", r#trait.ident),
        generics: r#trait.generics.clone(),
        colon_token: r#trait.colon_token,
        supertraits: r#trait.supertraits.clone(),
        brace_token: r#trait.brace_token,
        items: with_storage_items,
    };
    let impl_generics = r#trait.generics.clone();
    // let impl_trait = ItemImpl {
    //     attrs: vec![],
    //     defaultness: None,
    //     unsafety: r#trait.unsafety,
    //     impl_token: Default::default(),
    //     generics: impl_generics,
    //     trait_: Some(parse_quote!(__Trait for)),
    //     self_ty: todo!(),
    //     brace_token: Default::default(),
    //     items: vec![],
    // };
    Ok(quote! {
        #r#trait
        #with_storage_trait
    })
}

fn impl_return(method: &TraitItemFn) -> Option<&TypeImplTrait> {
    let ret = try_match!(&method.sig.output, ReturnType::Type(_, ty) => ty)?;
    try_match!(ret.as_ref(), Type::ImplTrait(ret) => ret)
}

fn new_method(signature: Signature) -> TraitItemFn {
    TraitItemFn {
        attrs: vec![],
        sig: signature,
        default: None,
        semi_token: None,
    }
}

fn plop(with_storage_items: &mut Vec<TraitItem>, method: &TraitItemFn, ret: &TypeImplTrait) {
    let dyn_lt = Lifetime::new("'dyn_lt", Span::call_site());
    let storage_lt = Lifetime::new("'__storage", Span::call_site());
    let storage_param = Ident::new("__storage", Span::call_site());
    let mut captured = CapturedLifetimes::new(&dyn_lt, ret, &method.sig.generics);
    let dyn_ret = TypeTraitObject {
        dyn_token: Some(Token![dyn](ret.impl_token.span)),
        bounds: (ret.bounds.iter())
            .filter(|b| matches!(b, TypeParamBound::Trait(_)))
            .map(|b| {
                let mut bound = b.clone();
                captured.visit_type_param_bound_mut(&mut bound);
                bound
            })
            .chain([TypeParamBound::Lifetime(dyn_lt.clone())])
            .collect(),
    };
    let fut_out = future_output(ret);
    let with_storage_name = format_ident!("{}_with_storage", method.sig.ident);
    let mut with_storage_sig = method.sig.clone();
    with_storage_sig.ident = with_storage_name.clone();
    (with_storage_sig.generics.params.iter_mut())
        .for_each(|param| captured.visit_generic_param_mut(param));
    with_storage_sig.generics.params.extend(
        [&dyn_lt, &storage_lt].map(|lt| GenericParam::Lifetime(LifetimeParam::new(lt.clone()))),
    );
    (with_storage_sig.inputs.iter_mut()).for_each(|arg| captured.visit_fn_arg_mut(arg));
    let mut storage_type = quote!(&#storage_lt mut ::core::option::Option<::dyn_utils::storage::DynStorage<::dyn_utils::DefaultStorage, ::dyn_utils::private::DynVTable, #dyn_ret>>);
    let mut storage_dyn_ret = quote!(&#storage_lt mut (#dyn_ret));
    if fut_out.is_some() {
        storage_type = quote!(::core::pin::Pin<#storage_type>);
        storage_dyn_ret = quote!(::core::pin::Pin<#storage_dyn_ret>);
    }
    (with_storage_sig.inputs).push(parse_quote!(#storage_param: #storage_type));
    with_storage_sig.output = parse_quote!(-> #storage_dyn_ret);
    with_storage_items.push(new_method(with_storage_sig).into());
    if let Some(fut_out) = fut_out {
        let async_sig = default_async_method(method, &with_storage_name, &storage_param, fut_out);
        with_storage_items.push(async_sig.into());
    }
}

fn default_async_method(
    method: &TraitItemFn,
    with_storage_name: &Ident,
    storage_param: &Ident,
    fut_out: &Type,
) -> TraitItemFn {
    let mut method = new_method(method.sig.clone());
    method.sig.asyncness = Some(Default::default());
    method.sig.output = ReturnType::Type(Default::default(), Box::new(fut_out.clone()));
    add_sized_bound(&mut method);
    let generics = method
        .sig
        .generics
        .params
        .iter()
        .filter(|p| !matches!(p, &GenericParam::Lifetime(_)));
    let args = method.sig.inputs.iter().map(|arg| match arg {
        FnArg::Receiver(recv) => recv.self_token.to_token_stream(),
        FnArg::Typed(arg) => arg.pat.to_token_stream(),
    });
    method.default = Some(parse_quote!({
        let #storage_param = ::core::pin::pin!(None);
        Self::#with_storage_name::<#(#generics)*>(#(#args,)* #storage_param).await
    }));
    method
}

struct CapturedLifetimes {
    dyn_lt: Lifetime,
    captured: HashSet<Lifetime>,
}

impl CapturedLifetimes {
    fn new(dyn_lt: &Lifetime, ret: &TypeImplTrait, generics: &Generics) -> Self {
        Self {
            dyn_lt: dyn_lt.clone(),
            captured: match (ret.bounds.iter())
                .find_map(try_match!(TypeParamBound::PreciseCapture(c) => c))
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
        if i.ident == "_" && self.captured.contains(i) {
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
            *lt = Some(parse_quote!('_));
        }
        syn::visit_mut::visit_receiver_mut(self, i);
    }

    fn visit_type_reference_mut(&mut self, i: &mut TypeReference) {
        if i.lifetime.is_none() {
            i.lifetime = Some(parse_quote!('_));
        }
        syn::visit_mut::visit_type_reference_mut(self, i);
    }
}

fn add_sized_bound(method: &mut TraitItemFn) {
    let where_clause = method.sig.generics.where_clause.insert(parse_quote!(where));
    where_clause.predicates.push(parse_quote!(Self: Sized))
}

fn future_output(ty: &TypeImplTrait) -> Option<&Type> {
    let path = (ty.bounds.iter()).find_map(try_match!(TypeParamBound::Trait(t) => &t.path))?;
    let future = path.segments.last().filter(|s| s.ident == "Future")?;
    let args = try_match!(&future.arguments, PathArguments::AngleBracketed(args) => &args.args)?;
    args.iter()
        .find_map(try_match!(GenericArgument::AssocType(t) if t.ident == "Output" => &t.ty))
}
