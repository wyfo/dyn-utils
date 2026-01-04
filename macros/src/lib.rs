#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use heck::ToPascalCase;
use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    FnArg, GenericParam, ImplItem, ImplItemFn, ItemTrait, Path, PathSegment, Token, TraitItem,
    TraitItemFn, Type, parse_macro_input, parse_quote, parse_quote_spanned, punctuated::Punctuated,
    spanned::Spanned, visit_mut, visit_mut::VisitMut,
};

use crate::{
    macros::{bail, try_match},
    methods::{
        dyn_method, handle_async_method, impl_method, is_dispatchable, is_sync_constant,
        parse_method_attrs, sync_method, try_sync_dyn_method, try_sync_impl_method,
    },
    utils::{return_type, to_impl_method},
};

mod macros;
mod methods;
mod utils;

#[proc_macro_attribute]
pub fn dyn_trait(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let r#trait = parse_macro_input!(item as ItemTrait);
    let mut remote = None;
    let mut dyn_trait = format_ident!("{}Dyn", r#trait.ident);
    let args_parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("remote") {
            meta.input.parse::<Token![=]>()?;
            remote = Some(meta.input.parse()?);
        } else {
            dyn_trait = meta.path.require_ident()?.clone();
        }
        Ok(())
    });
    parse_macro_input!(args with args_parser);
    dyn_trait_impl(r#trait, dyn_trait, remote)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn dyn_trait_impl(
    mut r#trait: ItemTrait,
    dyn_trait: Ident,
    remote: Option<Path>,
) -> syn::Result<TokenStream> {
    let include_trait = remote.is_none();
    let dyn_trait_attrs = r#trait
        .attrs
        .extract_if(.., |attr| attr.path().is_ident("dyn_utils"))
        .map(|attr| {
            let mut new_attr = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("dyn_storage") {
                    let remain: TokenStream = meta.input.parse().unwrap();
                    new_attr = Some(quote!(#[::dyn_utils::dyn_storage #remain]));
                } else {
                    bail!(meta.path, "unknown attribute");
                }
                Ok(())
            })?;
            Ok(new_attr.unwrap())
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let remote = remote.unwrap_or_else(|| r#trait.ident.clone().into());
    let mut dyn_items = Vec::new();
    let mut storages = Vec::<GenericParam>::new();
    let mut impl_items = Vec::<ImplItem>::new();
    let mut additional_trait_items = Vec::new();
    for item in r#trait.items.iter_mut() {
        match item {
            TraitItem::Type(ty) if ty.generics.params.is_empty() => {
                let (impl_generics, ty_generics, where_clause) = ty.generics.split_for_impl();
                let ty_name = &ty.ident;
                impl_items.push(parse_quote!(type #ty_name #impl_generics = <__Dyn as #remote>::#ty_name #ty_generics #where_clause;));
                dyn_items.push(item.clone());
            }
            TraitItem::Fn(method) if is_dispatchable(method) => {
                let attrs = parse_method_attrs(method)?;
                if attrs.send() {
                    handle_async_method(method, &attrs)?;
                }
                let mut method = TraitItemFn {
                    attrs: method.attrs.clone(),
                    sig: method.sig.clone(),
                    default: None,
                    semi_token: None,
                };
                if !attrs.send() {
                    handle_async_method(&mut method, &attrs)?;
                }
                if let Some(ret) = return_type(&method).and_then(try_match!(Type::ImplTrait)) {
                    let storage =
                        format_ident!("__Storage{}", method.sig.ident.to_string().to_pascal_case());
                    let default_storage = attrs.storage();
                    storages.push(parse_quote_spanned!(default_storage.span() => #storage: ::dyn_utils::storage::Storage = #default_storage));
                    let dyn_method = dyn_method(&method, ret, &storage);
                    let impl_method = impl_method(&dyn_method, true);
                    if attrs.try_sync() {
                        additional_trait_items.push(sync_method(&method, ret)?.into());
                        let is_sync = is_sync_constant(&method.sig, false);
                        additional_trait_items.push(parse_quote!(#[doc(hidden)] #is_sync));
                        dyn_items.push(try_sync_dyn_method(&dyn_method).into());
                        impl_items.push(try_sync_impl_method(&impl_method).into());
                    }
                    dyn_items.push(dyn_method.into());
                    impl_items.push(impl_method.into());
                } else {
                    attrs.check_no_attr()?;
                    impl_items.push(impl_method(&method, false).into());
                    dyn_items.push(method.into());
                }
            }
            _ => {}
        }
    }
    r#trait.items.extend(additional_trait_items);
    let unsafety = &r#trait.unsafety;
    let vis = &r#trait.vis;
    let supertraits = &r#trait.supertraits;
    let (_, trait_generics_ty, where_clause) = r#trait.generics.split_for_impl();
    let mut dyn_generics = r#trait.generics.clone();
    dyn_generics.params.extend(storages);
    let (_, dyn_generics_ty, _) = dyn_generics.split_for_impl();
    let mut impl_generics = dyn_generics.clone();
    impl_generics
        .params
        .push(parse_quote!(__Dyn: #remote #trait_generics_ty));
    let (impl_generics, _, _) = impl_generics.split_for_impl();
    let r#trait = include_trait.then_some(&r#trait);
    Ok(quote! {
        #r#trait
        #(#dyn_trait_attrs)*
        #vis #unsafety trait #dyn_trait #dyn_generics #supertraits #where_clause { #(#dyn_items)* }
        #unsafety impl #impl_generics #dyn_trait #dyn_generics_ty for __Dyn #where_clause { #(#impl_items)* }
    })
}

#[proc_macro_attribute]
pub fn sync(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    sync_impl(parse_macro_input!(item as ImplItemFn))
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn sync_impl(method: ImplItemFn) -> syn::Result<TokenStream> {
    if method.sig.asyncness.is_none() {
        bail!(
            method.sig.fn_token, // Because nightly doesn't give the same span for `method`
            "`dyn_utils::sync` must be used on async method"
        );
    }
    let method_name = &method.sig.ident;
    let mut sync_method = method.clone();
    sync_method.sig.asyncness = None;
    sync_method.sig.ident = format_ident!("{method_name}_sync");
    let is_sync = is_sync_constant(&method.sig, true);
    Ok(quote! {
        #method
        #sync_method
        #is_sync
    })
}

#[proc_macro_attribute]
pub fn dyn_storage(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let r#trait = parse_macro_input!(item as ItemTrait);
    let mut remote = None;
    let mut bounds = Punctuated::new();
    let mut crate_path = parse_quote!(::dyn_utils);
    let args_parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("remote") {
            meta.input.parse::<Token![=]>()?;
            remote = Some(meta.input.parse()?);
        } else if meta.path.is_ident("bounds") {
            meta.input.parse::<Token![=]>()?;
            bounds = Punctuated::parse_terminated(meta.input)?;
        } else if meta.path.is_ident("crate") {
            meta.input.parse::<Token![=]>()?;
            crate_path = meta.input.parse()?;
        } else {
            bail!(meta.path, "unknown attribute");
        }
        Ok(())
    });
    parse_macro_input!(args with args_parser);
    dyn_storage_impl(r#trait, remote, bounds, crate_path)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn dyn_storage_impl(
    r#trait: ItemTrait,
    remote: Option<Path>,
    bounds: Punctuated<Path, Token![+]>,
    crate_path: Path,
) -> syn::Result<TokenStream> {
    let include_trait = remote.is_none()
        || r#trait.attrs.iter().any(|attr| {
            attr.path()
                .segments
                .last()
                .is_some_and(|s| s.ident == "dyn_storage")
        });
    let remote = remote.unwrap_or_else(|| r#trait.ident.clone().into());
    let (_, trait_generics, where_clause) = r#trait.generics.split_for_impl();
    let remote_with_args = quote!(#remote #trait_generics);
    let mut methods = Vec::new();
    let mut types = Vec::new();
    for item in r#trait.items.iter() {
        match item {
            TraitItem::Fn(method) => {
                if !is_dispatchable(method) {
                    bail!(method.sig.fn_token, "method is not dispatchable");
                }
                methods.push(method);
            }
            TraitItem::Type(ty) => {
                if !ty.generics.params.is_empty() {
                    bail!(ty.ident, "generic associated type is not supported");
                }
                types.push((format_ident!("__Type{}", ty.ident), ty));
            }
            _ => bail!(item, "unsupported item"),
        }
    }
    let dyn_trait_args = r#trait
        .generics
        .params
        .iter()
        .map(|param| match param {
            GenericParam::Lifetime(p) => p.lifetime.to_token_stream(),
            GenericParam::Type(p) => p.ident.to_token_stream(),
            GenericParam::Const(p) => p.ident.to_token_stream(),
        })
        .chain(types.iter().map(|(ty_arg, ty)| {
            let ty_name = &ty.ident;
            quote!(#ty_name = #ty_arg)
        }));
    let mut dyn_trait = quote!(#remote<#(#dyn_trait_args,)*> + '__lt);
    if !bounds.is_empty() {
        dyn_trait.extend(quote!(+ #bounds));
    }
    let mut generics = r#trait.generics.params.iter().cloned().collect::<Vec<_>>();
    generics.iter_mut().for_each(|param| match param {
        GenericParam::Lifetime(_) => {}
        GenericParam::Type(p) => p.default = None,
        GenericParam::Const(p) => p.default = None,
    });
    generics.insert(0, parse_quote!('__lt));
    generics.extend(types.iter().map(|(ty_arg, ty)| -> GenericParam {
        let bounds = &ty.bounds;
        // https://github.com/dtolnay/syn/issues/1952
        if bounds.is_empty() {
            parse_quote!(#ty_arg)
        } else {
            parse_quote!(#ty_arg: #bounds)
        }
    }));
    let method_names = methods.iter().map(|method| method.sig.ident.clone());
    let vtable_methods = methods.iter().map(|method| {
        let method_name = &method.sig.ident;
        let args = (method.sig.inputs.iter())
            .filter_map(try_match!(FnArg::Typed(arg) => &arg.pat))
            .collect::<Vec<_>>();
        let self_as = match VTableReceiver::new(method) {
            VTableReceiver::Ref => quote!(as_ref),
            VTableReceiver::Mut => quote!(as_mut),
            VTableReceiver::Pinned => quote!(as_pinned_mut),
        };
        let method_sig = vtable_method_signature(method, true);
        quote! {
            #[allow(clippy::useless_transmute)]
            #method_name: unsafe {
                ::core::mem::transmute::<#method_sig ,unsafe fn()>(
                    // transmute to erase lifetime
                    |__self, #(#args,)*| ::core::mem::transmute(
                        __Dyn::#method_name(__self.#self_as(), #(#args,)*)
                    )
                )
            }
        }
    });
    let method_impls = methods.iter().map(|method| {
        let method_name = &method.sig.ident;
        let self_as = match VTableReceiver::new(method) {
            VTableReceiver::Ref => quote!(inner),
            VTableReceiver::Mut => quote!(inner_mut),
            VTableReceiver::Pinned => quote!(inner_pinned_mut),
        };
        let args = (method.sig.inputs.iter())
            .filter_map(try_match!(FnArg::Typed(arg) => &arg.pat))
            .collect::<Vec<_>>();
        let method_sig = vtable_method_signature(method, false);
        let block = parse_quote!({ unsafe {
            ::core::mem::transmute::<unsafe fn(), #method_sig>(self.vtable().#method_name)(self.#self_as(), #(#args,)*)
        } });
        to_impl_method(method, block)
    });
    let type_impls = types.iter().map(|(ty_arg, ty)| {
        let ty_name = &ty.ident;
        quote!(type #ty_name = #ty_arg;)
    });
    let r#trait = include_trait.then_some(&r#trait);
    Ok(quote! {
        #r#trait
        const _: () = {
            #[derive(Debug)]
            pub struct __VTable {
                __drop_in_place: Option<unsafe fn(::core::ptr::NonNull<()>)>,
                __layout: ::core::alloc::Layout,
                #(#method_names: unsafe fn()),*
            }

            impl<#(#generics,)*> #crate_path::private::DynTrait for dyn #dyn_trait #where_clause {
                type VTable = __VTable;
                fn drop_in_place(vtable: &Self::VTable) -> Option<unsafe fn(core::ptr::NonNull<()>)> {
                    vtable.__drop_in_place
                }
                fn layout(vtable: &Self::VTable) -> core::alloc::Layout {
                    vtable.__layout
                }
            }

            unsafe impl<#(#generics,)* __Dyn: #dyn_trait> #crate_path::private::NewVTable<__Dyn>
                for dyn #dyn_trait #where_clause
            {
                fn new_vtable<__Storage: #crate_path::storage::Storage>() -> &'static Self::VTable {
                    &const {
                        __VTable {
                            __drop_in_place: const {
                                if core::mem::needs_drop::<__Dyn>() {
                                    Some(|ptr_mut| unsafe { ptr_mut.cast::<__Dyn>().drop_in_place() })
                                } else {
                                    None
                                }
                            },
                            __layout: const { core::alloc::Layout::new::<__Dyn>() },
                            #(#vtable_methods,)*
                        }
                    }
                }
            }

            impl<#(#generics,)* __Storage: #crate_path::storage::Storage> #remote_with_args
                for #crate_path::DynStorage<dyn #dyn_trait, __Storage> #where_clause
            {
                #(#type_impls)*
                #(#method_impls)*
            }
        };
    })
}

enum VTableReceiver {
    Ref,
    Mut,
    Pinned,
}

impl VTableReceiver {
    fn new(method: &TraitItemFn) -> Self {
        let recv = method.sig.receiver().unwrap();
        if recv.reference.is_none() {
            Self::Pinned
        } else if recv.mutability.is_some() {
            Self::Mut
        } else {
            Self::Ref
        }
    }
}

struct ReplaceSelfWithDyn;

impl VisitMut for ReplaceSelfWithDyn {
    fn visit_path_segment_mut(&mut self, i: &mut PathSegment) {
        if i.ident == "Self" {
            i.ident = parse_quote!(__Dyn);
        }
        visit_mut::visit_path_segment_mut(self, i);
    }
}

fn vtable_method_signature(method: &TraitItemFn, new_vtable: bool) -> TokenStream {
    let unsafety = &method.sig.unsafety;
    let storage = match VTableReceiver::new(method) {
        VTableReceiver::Ref => quote!(&__Storage),
        VTableReceiver::Mut => quote!(&mut __Storage),
        VTableReceiver::Pinned => quote!(::core::pin::Pin<&mut __Storage>),
    };
    let params = method
        .sig
        .inputs
        .iter()
        .filter_map(try_match!(FnArg::Typed(arg) => arg.ty.clone()))
        .map(|mut ty| {
            if new_vtable {
                ReplaceSelfWithDyn.visit_type_mut(&mut ty);
            }
            ty
        });
    let mut output = method.sig.output.clone();
    if new_vtable {
        ReplaceSelfWithDyn.visit_return_type_mut(&mut output);
    }
    let lifetimes = method
        .sig
        .generics
        .lifetimes()
        .map(|l| &l.lifetime)
        .take(if new_vtable { usize::MAX } else { 0 });
    quote!(for<#(#lifetimes,)*> #unsafety fn(#storage, #(#params,)*) #output)
}
