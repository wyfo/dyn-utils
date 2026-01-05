use std::collections::HashSet;

use heck::ToPascalCase;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{
    CapturedParam, GenericParam, Generics, ImplItem, ImplItemFn, ItemTrait, Lifetime,
    LifetimeParam, Path, Receiver, Signature, Token, TraitItem, TraitItemConst, TraitItemFn,
    TraitItemType, Type, TypeImplTrait, TypeParamBound, TypeReference, TypeTraitObject,
    WherePredicate, meta::ParseNestedMeta, parse_quote, parse_quote_spanned, spanned::Spanned,
    visit_mut::VisitMut,
};

use crate::{
    MacroArgs, crate_name,
    macros::{bail, bail_method, fields, try_match},
    sync::{is_sync_const, sync_fn, try_sync_fn},
    utils::{
        IteratorExt, PatternAsArg, fn_args, future_output, impl_method, is_dispatchable,
        is_not_generic, return_type,
    },
};

#[derive(Default)]
pub(super) struct DynTraitOpts {
    crate_: Option<Path>,
    remote: Option<Path>,
    name_template: Option<String>,
}

impl MacroArgs for DynTraitOpts {
    fn parse_meta(&mut self, meta: ParseNestedMeta) -> syn::Result<()> {
        if meta.path.is_ident("crate") {
            meta.input.parse::<Token![=]>()?;
            self.crate_ = Some(meta.input.parse()?);
        } else if meta.path.is_ident("remote") {
            meta.input.parse::<Token![=]>()?;
            self.remote = Some(meta.input.parse()?);
        } else if meta.path.is_ident("trait") {
            meta.input.parse::<Token![=]>()?;
            self.name_template = Some(if meta.input.peek(syn::Ident) {
                meta.input.parse::<Ident>()?.to_string()
            } else if meta.input.peek(syn::LitStr) {
                meta.input.parse::<syn::LitStr>()?.value()
            } else {
                bail!(meta.input.span(), "invalid trait name");
            });
        } else {
            bail!(meta.path, "unknown attribute");
        }
        Ok(())
    }
}

pub(super) fn dyn_trait_impl(
    mut r#trait: ItemTrait,
    opts: DynTraitOpts,
) -> syn::Result<TokenStream> {
    let mut dyn_trait = DynTrait::new(&r#trait, opts);
    let dyn_trait_attrs = extract_dyn_trait_attrs(&mut r#trait)?;
    for item in r#trait.items.iter_mut() {
        match item {
            TraitItem::Type(ty) if is_not_generic(ty) => {
                dyn_trait.parse_type(ty);
            }
            TraitItem::Fn(method) if is_dispatchable(method) => {
                dyn_trait.parse_method(method)?;
            }
            _ => {}
        }
    }
    r#trait.items.extend(dyn_trait.additional_trait_items);

    let opt_trait = dyn_trait.include_trait.then_some(&r#trait);
    fields!(dyn_trait => remote, dyn_trait_name, dyn_items, impl_items);
    fields!(r#trait => ident, unsafety, vis, supertraits);
    let (_, trait_ty_gen, where_clause) = r#trait.generics.split_for_impl();
    let dyn_param = parse_quote!(__Dyn: #remote #trait_ty_gen);

    let mut dyn_generics = r#trait.generics.clone();
    dyn_generics.params.extend(dyn_trait.generic_storages);
    let mut impl_generics = dyn_generics.clone();
    impl_generics.params.push(dyn_param);
    let (_, dyn_ty_gen, _) = dyn_generics.split_for_impl();
    let (impl_impl_gen, _, _) = impl_generics.split_for_impl();

    Ok(quote! {
        #opt_trait

        #[doc = "Dyn-compatible implementation of"]
        #[doc = ::core::concat!("[`", stringify!(#ident), "`](", stringify!(#remote), ").")]
        #(#dyn_trait_attrs)*
        #vis #unsafety trait #dyn_trait_name #dyn_generics #supertraits #where_clause { #(#dyn_items)* }

        #unsafety impl #impl_impl_gen #dyn_trait_name #dyn_ty_gen for __Dyn #where_clause { #(#impl_items)* }
    })
}

struct DynTrait {
    include_trait: bool,
    dyn_trait_name: Ident,
    crate_: Path,
    remote: Path,
    trait_generics: Vec<Ident>,
    additional_trait_items: Vec<TraitItem>,
    dyn_items: Vec<TraitItem>,
    impl_items: Vec<ImplItem>,
    generic_storages: Vec<GenericParam>,
}

impl DynTrait {
    fn new(r#trait: &ItemTrait, opts: DynTraitOpts) -> Self {
        let template = opts.name_template.as_deref().unwrap_or("Dyn{}");
        Self {
            include_trait: opts.remote.is_none(),
            dyn_trait_name: format_ident!("{}", template.replace("{}", &r#trait.ident.to_string())),
            crate_: opts.crate_.unwrap_or_else(crate_name),
            remote: opts.remote.unwrap_or_else(|| r#trait.ident.clone().into()),
            trait_generics: (r#trait.generics.type_params())
                .map(|t| t.ident.clone())
                .collect(),
            additional_trait_items: Vec::new(),
            dyn_items: Vec::new(),
            impl_items: Vec::new(),
            generic_storages: Vec::new(),
        }
    }

    fn parse_type(&mut self, ty: &TraitItemType) {
        self.dyn_items.push(ty.clone().into());
        let remote = &self.remote;
        let ty_name = &ty.ident;
        self.impl_items
            .push(parse_quote!(type #ty_name = <__Dyn as #remote>::#ty_name;));
    }

    fn parse_method(&mut self, method: &mut TraitItemFn) -> syn::Result<()> {
        let attrs = MethodAttrs::parse(method)?;
        let dyn_method = DynMethod::new(&self.crate_, &self.trait_generics, method);
        self.generic_storages
            .extend(dyn_method.generic_storage(attrs.storage));
        if attrs.try_sync {
            self.additional_trait_items
                .push(dyn_method.sync_method()?.into());
            self.additional_trait_items
                .push(dyn_method.is_sync_const().into());
            self.impl_items.push(dyn_method.try_sync_impl().into());
            self.dyn_items.push(dyn_method.try_sync_method().into());
        }
        self.impl_items.push(dyn_method.impl_method().into());
        self.dyn_items.push(dyn_method.dyn_method.into());
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct MethodAttrs {
    storage: Option<Path>,
    try_sync: bool,
}

impl MethodAttrs {
    fn parse(method: &mut TraitItemFn) -> syn::Result<Self> {
        let has_rpit = method.sig.asyncness.is_some()
            || return_type(&method.sig)
                .and_then(try_match!(Type::ImplTrait))
                .is_some();
        let mut attrs = Self::default();
        for attr in (method.attrs).extract_if(.., |attr| attr.path().is_ident("dyn_trait")) {
            if !has_rpit {
                let err = "attribute must be used on a method with Return Position Impl Trait";
                bail!(attr.meta.path(), err);
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("storage") {
                    meta.input.parse::<Token![=]>()?;
                    attrs.storage = Some(meta.input.parse()?);
                } else if meta.path.is_ident("try_sync") {
                    attrs.try_sync = true
                } else {
                    bail!(meta.path, "unknown attribute");
                }
                Ok(())
            })?;
        }
        Ok(attrs)
    }
}

struct DynMethod<'a> {
    orig_sig: &'a Signature,
    crate_: &'a Path,
    dyn_method: TraitItemFn,
    rpit: Option<TypeImplTrait>,
    storage: Option<Ident>,
}

impl<'a> DynMethod<'a> {
    fn new(crate_: &'a Path, trait_generics: &[Ident], method: &'a TraitItemFn) -> Self {
        let orig_sig = &method.sig;
        let mut method = TraitItemFn {
            attrs: method.attrs.clone(),
            sig: method.sig.clone(),
            default: None,
            semi_token: None,
        };
        // patterns are not allowed without default
        PatternAsArg.visit_signature_mut(&mut method.sig);
        // convert async fn to RPIT
        if method.sig.asyncness.is_some() {
            method.sig.asyncness = None;
            let output = return_type(&method.sig).map_or_else(|| quote!(()), |ty| quote!(#ty));
            method.sig.output = parse_quote!(-> impl Future<Output = #output>);
        }
        let rpit = return_type(&method.sig)
            .and_then(try_match!(Type::ImplTrait))
            .cloned();
        let storage = (rpit.is_some())
            .then(|| format_ident!("__Storage{}", method.sig.ident.to_string().to_pascal_case()));
        if let Some(rpit) = &rpit {
            Self::update_dyn_signature(crate_, trait_generics, &mut method.sig, rpit, &storage);
        }
        Self {
            orig_sig,
            crate_,
            dyn_method: method,
            rpit,
            storage,
        }
    }

    fn update_dyn_signature(
        crate_: &Path,
        trait_generics: &[Ident],
        sig: &mut Signature,
        rpit: &TypeImplTrait,
        storage: &Option<Ident>,
    ) {
        let mut captured = CapturedLifetimes::new(rpit, &sig.generics);
        let dyn_ret = TypeTraitObject {
            dyn_token: Some(Default::default()),
            bounds: (rpit.bounds.iter())
                .filter(|b| matches!(b, TypeParamBound::Trait(_)))
                .cloned()
                .update(|bound| captured.visit_type_param_bound_mut(bound))
                .chain([parse_quote!('__dyn)])
                .collect(),
        };
        // All type parameters are captured, so their lifetime must be bounded
        sig.generics.make_where_clause().predicates.extend(
            (trait_generics.iter())
                .map(|p| parse_quote!(#p: '__dyn))
                .chain::<[WherePredicate; 1]>([parse_quote!(Self: '__dyn)]),
        );
        (sig.generics.params.iter_mut()).for_each(|param| captured.visit_generic_param_mut(param));
        sig.generics.params.insert(0, parse_quote!('__dyn));
        (sig.inputs.iter_mut()).for_each(|arg| captured.visit_fn_arg_mut(arg));
        sig.output = parse_quote!(-> #crate_::DynStorage<#dyn_ret, #storage>);
    }

    fn generic_storage(&self, default_storage: Option<Path>) -> Option<GenericParam> {
        let crate_ = &self.crate_;
        let storage = self.storage.as_ref()?;
        let default_storage =
            default_storage.unwrap_or_else(|| parse_quote!(#crate_::DefaultStorage));
        Some(parse_quote_spanned! { default_storage.span() =>
            #storage: #crate_::storage::Storage = #default_storage
        })
    }

    fn impl_method(&self) -> ImplItemFn {
        let crate_ = &self.crate_;
        let method_name = &self.orig_sig.ident;
        let args = fn_args(self.orig_sig);
        let call = quote!(__Dyn::#method_name(#(#args,)*));
        let block = if self.rpit.is_some() {
            parse_quote!({ #crate_::DynStorage::new(#call) })
        } else {
            parse_quote!({ #call })
        };
        impl_method(self.dyn_method.sig.clone(), block)
    }

    fn sync_method(&self) -> syn::Result<TraitItemFn> {
        let Some(output) = self.rpit.as_ref().and_then(future_output) else {
            bail_method!(self.dyn_method, "`try_sync` must be used on async methods");
        };
        let args = fn_args(self.orig_sig).skip(1);
        Ok(TraitItemFn {
            attrs: vec![parse_quote!(#[doc(hidden)])],
            sig: Signature {
                asyncness: None,
                ident: sync_fn(self.orig_sig),
                output: parse_quote!(-> #output),
                ..self.orig_sig.clone()
            },
            default: Some(parse_quote!({ #(let _ = #args;)* ::core::unimplemented!() })),
            semi_token: None,
        })
    }

    fn is_sync_const(&self) -> TraitItemConst {
        let is_sync = is_sync_const(self.orig_sig);
        parse_quote!(#[doc(hidden)] const #is_sync: bool = false;)
    }

    fn try_sync_signature(&self) -> Signature {
        let crate_ = &self.crate_;
        let mut signature = self.dyn_method.sig.clone();
        signature.ident = try_sync_fn(self.orig_sig);
        let output = return_type(&self.dyn_method.sig).unwrap();
        signature.output = parse_quote!(-> #crate_::TrySync<#output>);
        signature
    }

    fn try_sync_method(&self) -> TraitItemFn {
        let mut method = self.dyn_method.clone();
        method.sig = self.try_sync_signature();
        method
    }

    fn try_sync_impl(&self) -> ImplItemFn {
        let crate_ = &self.crate_;
        let is_sync = is_sync_const(self.orig_sig);
        let sync_method = sync_fn(self.orig_sig);
        let async_method = &self.orig_sig.ident;
        let args = fn_args(self.orig_sig).collect_vec();
        let block = parse_quote!({
           if __Dyn::#is_sync {
                #crate_::TrySync::Sync(__Dyn::#sync_method(#(#args),*))
            } else {
                #crate_::TrySync::Async(#crate_::DynStorage::new(__Dyn::#async_method(#(#args,)*)))
            }
        });
        impl_method(self.try_sync_signature(), block)
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

fn extract_dyn_trait_attrs(r#trait: &mut ItemTrait) -> syn::Result<Vec<TokenStream>> {
    (r#trait.attrs)
        .extract_if(.., |attr| attr.path().is_ident("dyn_trait"))
        .map(|attr| {
            let meta = &attr.meta.require_list()?.tokens;
            Ok(quote!(#[#meta]))
        })
        .collect()
}
