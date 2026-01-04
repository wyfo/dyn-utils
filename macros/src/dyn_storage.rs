use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    FnArg, GenericParam, ItemTrait, Path, PathSegment, Token, TraitItem, TraitItemFn,
    meta::ParseNestedMeta, parse_quote, punctuated::Punctuated, visit_mut, visit_mut::VisitMut,
};

use crate::{
    macros::{bail, try_match},
    utils::{IteratorExt, fn_args, impl_method, is_dispatchable, last_segments, respan},
};

pub(super) struct DynStorageOpts {
    dyn_utils: Path,
    bounds: Punctuated<Path, Token![+]>,
    remote: Option<Path>,
}

impl DynStorageOpts {
    pub(super) fn new() -> Self {
        Self {
            dyn_utils: parse_quote!(::dyn_utils),
            bounds: Punctuated::new(),
            remote: None,
        }
    }

    pub(super) fn parse_meta(&mut self, meta: ParseNestedMeta) -> syn::Result<()> {
        if meta.path.is_ident("bounds") {
            meta.input.parse::<Token![=]>()?;
            self.bounds = Punctuated::parse_terminated(meta.input)?;
        } else if meta.path.is_ident("crate") {
            meta.input.parse::<Token![=]>()?;
            self.dyn_utils = meta.input.parse()?;
        } else if meta.path.is_ident("remote") {
            meta.input.parse::<Token![=]>()?;
            self.remote = Some(meta.input.parse()?);
        } else {
            bail!(meta.path, "unknown attribute");
        }
        Ok(())
    }
}

pub(super) fn dyn_storage_impl(
    r#trait: ItemTrait,
    opts: DynStorageOpts,
) -> syn::Result<TokenStream> {
    let DynStorageOpts {
        bounds,
        dyn_utils,
        remote,
    } = opts;
    let include_trait = remote.is_none()
        || (r#trait.attrs.iter()).any(|attr| last_segments(attr.path(), "dyn_storage").is_some());
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
    let mut generics = r#trait.generics.params.iter().cloned().collect_vec();
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
        let args = fn_args(&method.sig).skip(1).collect_vec();
        let erased_args = args.iter().map(|arg| quote!(::core::mem::transmute(#arg)));
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
                        __Dyn::#method_name(__self.#self_as(), #(#erased_args,)*)
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
        let args = fn_args(&method.sig).skip(1);
        let method_sig = vtable_method_signature(method, false);
        let block = parse_quote!({ unsafe {
            ::core::mem::transmute::<unsafe fn(), #method_sig>(self.vtable().#method_name)(self.#self_as(), #(#args,)*)
        } });
        impl_method(method.sig.clone(), block)
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

            impl<#(#generics,)*> #dyn_utils::private::DynTrait for dyn #dyn_trait #where_clause {
                type VTable = __VTable;
                fn drop_in_place(vtable: &Self::VTable) -> Option<unsafe fn(core::ptr::NonNull<()>)> {
                    vtable.__drop_in_place
                }
                fn layout(vtable: &Self::VTable) -> core::alloc::Layout {
                    vtable.__layout
                }
            }

            unsafe impl<#(#generics,)* __Dyn: #dyn_trait> #dyn_utils::private::NewVTable<__Dyn>
                for dyn #dyn_trait #where_clause
            {
                fn new_vtable<__Storage: #dyn_utils::storage::Storage>() -> &'static Self::VTable {
                    &const {
                        __VTable {
                            __drop_in_place: if core::mem::needs_drop::<__Dyn>() {
                                Some(|ptr_mut| unsafe { ptr_mut.cast::<__Dyn>().drop_in_place() })
                            } else {
                                None
                            },
                            __layout: const { core::alloc::Layout::new::<__Dyn>() },
                            #(#vtable_methods,)*
                        }
                    }
                }
            }

            impl<#(#generics,)* __Storage: #dyn_utils::storage::Storage> #remote_with_args
                for #dyn_utils::DynStorage<dyn #dyn_trait, __Storage> #where_clause
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
    // We don't care about self lifetime because it is erased anyway
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
        .update(|ty| {
            if new_vtable {
                ReplaceSelfWithDyn.visit_type_mut(ty);
            }
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
    let fn_ptr = quote!(for<#(#lifetimes,)*> #unsafety fn(#storage, #(#params,)*) #output);
    // because without it, RustRover highlight every type as unsafe code use
    respan(fn_ptr, Span::call_site())
}
