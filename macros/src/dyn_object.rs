use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use syn::{
    FnArg, GenericParam, ImplItemFn, ItemTrait, Path, PathSegment, Token, TraitItem, TraitItemFn,
    TraitItemType, meta::ParseNestedMeta, parse_quote, punctuated::Punctuated, visit_mut,
    visit_mut::VisitMut,
};

use crate::{
    MacroArgs, crate_name,
    macros::{bail, bail_method, fields, try_match},
    utils::{IteratorExt, fn_args, impl_method, is_dispatchable, last_segments, respan},
};

#[derive(Default)]
pub(super) struct DynObjectOps {
    bounds: Punctuated<Path, Token![+]>,
    crate_: Option<Path>,
    remote: Option<Path>,
}

impl MacroArgs for DynObjectOps {
    fn parse_meta(&mut self, meta: ParseNestedMeta) -> syn::Result<()> {
        if meta.path.is_ident("bounds") {
            meta.input.parse::<Token![=]>()?;
            self.bounds = Punctuated::parse_terminated(meta.input)?;
        } else if meta.path.is_ident("crate") {
            meta.input.parse::<Token![=]>()?;
            self.crate_ = Some(meta.input.parse()?);
        } else if meta.path.is_ident("remote") {
            meta.input.parse::<Token![=]>()?;
            self.remote = Some(meta.input.parse()?);
        } else {
            bail!(meta.path, "unknown attribute");
        }
        Ok(())
    }
}

pub(super) fn dyn_object_impl(r#trait: ItemTrait, opts: DynObjectOps) -> syn::Result<TokenStream> {
    let mut dyn_object = DynObject::new(&r#trait, opts);
    for item in r#trait.items.iter() {
        match item {
            TraitItem::Fn(method) => {
                if !is_dispatchable(method) {
                    bail_method!(method, "method is not dispatchable");
                }
                dyn_object.methods.push(method);
            }
            TraitItem::Type(ty) => {
                if !ty.generics.params.is_empty() {
                    bail!(ty.ident, "generic associated type is not supported");
                }
                let gen_param = format_ident!("__Type{}", ty.ident);
                dyn_object.types.push((gen_param, ty));
            }
            _ => bail!(item, "unsupported item"),
        }
    }
    fields!(dyn_object => crate_, remote);
    let dyn_trait = dyn_object.dyn_trait();
    let generics = dyn_object.generics();
    let vtable_fields = (dyn_object.methods.iter()).map(|m| dyn_object.vtable_field(m));
    let vtable_methods = (dyn_object.methods.iter()).map(|m| dyn_object.vtable_method(m));
    let impl_methods = (dyn_object.methods.iter()).map(|m| dyn_object.impl_method(m));
    let impl_types = (dyn_object.types.iter()).map(|t| dyn_object.impl_type(t));
    let (_, ty_gen, where_clause) = r#trait.generics.split_for_impl();
    let remote_with_args = quote!(#remote #ty_gen);
    let opt_trait = dyn_object.include_trait.then_some(&r#trait);
    Ok(quote! {
        #opt_trait

        const _: () = {
            #[derive(Debug)]
            pub struct __Vtable {
                __drop_in_place: Option<unsafe fn(::core::ptr::NonNull<()>)>,
                __layout: ::core::alloc::Layout,
                #(#vtable_fields,)*
            }

            impl<#(#generics,)*> #crate_::object::DynTrait for dyn #dyn_trait #where_clause {
                type Vtable = __Vtable;
                fn drop_in_place_fn(vtable: &Self::Vtable) -> Option<unsafe fn(core::ptr::NonNull<()>)> {
                    vtable.__drop_in_place
                }
                fn layout(vtable: &Self::Vtable) -> core::alloc::Layout {
                    vtable.__layout
                }
            }

            // SAFETY: vtable fields respect trait contract
            unsafe impl<#(#generics,)* __Dyn: #dyn_trait> #crate_::object::Vtable<__Dyn>
                for dyn #dyn_trait #where_clause
            {
                fn vtable<__Storage: #crate_::storage::Storage>() -> &'static Self::Vtable {
                    &const {
                        __Vtable {
                            __drop_in_place: <Self as #crate_::object::Vtable<__Dyn>>::DROP_IN_PLACE_FN,
                            __layout: core::alloc::Layout::new::<__Dyn>(),
                            #(#vtable_methods,)*
                        }
                    }
                }
            }

            impl<#(#generics,)* __Storage: #crate_::storage::Storage> #remote_with_args
                for #crate_::DynObject<dyn #dyn_trait, __Storage> #where_clause
            {
                #(#impl_types)*
                #(#impl_methods)*
            }
        };
    })
}

struct DynObject<'a> {
    r#trait: &'a ItemTrait,
    include_trait: bool,
    crate_: Path,
    remote: Path,
    bounds: Punctuated<Path, Token![+]>,
    types: Vec<(Ident, &'a TraitItemType)>,
    methods: Vec<&'a TraitItemFn>,
}

impl<'a> DynObject<'a> {
    fn new(r#trait: &'a ItemTrait, opts: DynObjectOps) -> Self {
        let has_dyn_object_attr = || {
            (r#trait.attrs.iter()).any(|attr| last_segments(attr.path(), "dyn_object").is_some())
        };
        Self {
            r#trait,
            include_trait: opts.remote.is_none() || has_dyn_object_attr(),
            crate_: opts.crate_.unwrap_or_else(crate_name),
            remote: opts.remote.unwrap_or_else(|| r#trait.ident.clone().into()),
            bounds: opts.bounds,
            types: Vec::new(),
            methods: Vec::new(),
        }
    }

    fn dyn_trait(&self) -> TokenStream {
        let dyn_trait_args = (self.r#trait.generics.params.iter())
            .map(|param| match param {
                GenericParam::Lifetime(p) => p.lifetime.to_token_stream(),
                GenericParam::Type(p) => p.ident.to_token_stream(),
                GenericParam::Const(p) => p.ident.to_token_stream(),
            })
            .chain(self.types.iter().map(|(ty_arg, ty)| {
                let ty_name = &ty.ident;
                quote!(#ty_name = #ty_arg)
            }));
        fields!(self => bounds, remote);
        let mut dyn_trait = quote!(#remote<#(#dyn_trait_args,)*> + '__lt);
        if !bounds.is_empty() {
            dyn_trait.extend(quote!(+ #bounds));
        }
        dyn_trait
    }

    fn generics(&self) -> Vec<GenericParam> {
        let mut generics = self.r#trait.generics.params.iter().cloned().collect_vec();
        generics.iter_mut().for_each(|param| match param {
            GenericParam::Lifetime(_) => {}
            GenericParam::Type(p) => p.default = None,
            GenericParam::Const(p) => p.default = None,
        });
        generics.insert(0, parse_quote!('__lt));
        generics.extend(self.types.iter().map(|(ty_arg, ty)| -> GenericParam {
            let bounds = &ty.bounds;
            // https://github.com/dtolnay/syn/issues/1952
            if bounds.is_empty() {
                parse_quote!(#ty_arg)
            } else {
                parse_quote!(#ty_arg: #bounds)
            }
        }));
        generics
    }

    fn vtable_field(&self, method: &TraitItemFn) -> TokenStream {
        let method_name = &method.sig.ident;
        quote!(#method_name: unsafe fn())
    }

    fn vtable_method(&self, method: &TraitItemFn) -> TokenStream {
        let method_name = &method.sig.ident;
        let args = fn_args(&method.sig).skip(1).collect_vec();
        let erased_args = args.iter().map(|arg| quote!(::core::mem::transmute(#arg)));
        let self_as = match VtableReceiver::new(method) {
            VtableReceiver::Ref => quote!(as_ref),
            VtableReceiver::Mut => quote!(as_mut),
            VtableReceiver::Pinned => quote!(as_pinned_mut),
        };
        let fn_ptr = vtable_fn_pointer(method, true);
        quote! {
            #[allow(
                clippy::missing_transmute_annotations,
                clippy::useless_transmute
            )]
            // SAFETY: transmutation are only used to erase lifetime,
            // the real lifetime being enforced in the trait implementation
            #method_name: unsafe {
                ::core::mem::transmute::<#fn_ptr ,unsafe fn()>(
                    |__self, #(#args,)*| ::core::mem::transmute(
                        __Dyn::#method_name(__self.#self_as(), #(#erased_args,)*)
                    )
                )
            }
        }
    }

    fn impl_method(&self, method: &TraitItemFn) -> ImplItemFn {
        let method_name = &method.sig.ident;
        let self_as = match VtableReceiver::new(method) {
            VtableReceiver::Ref => quote!(storage),
            VtableReceiver::Mut => quote!(storage_mut),
            VtableReceiver::Pinned => quote!(storage_pinned_mut),
        };
        let args = fn_args(&method.sig).skip(1);
        let fn_ptr = vtable_fn_pointer(method, false);
        // SAFETY: the vtable method has been initialized with the given type
        let block = parse_quote!({ unsafe {
            ::core::mem::transmute::<unsafe fn(), #fn_ptr>(self.vtable().#method_name)(
                self.#self_as(), #(#args,)*
            )
        } });
        impl_method(method.sig.clone(), block)
    }

    fn impl_type(&self, (ty_param, ty): &(Ident, &TraitItemType)) -> TokenStream {
        let ty_name = &ty.ident;
        quote!(type #ty_name = #ty_param;)
    }
}

enum VtableReceiver {
    Ref,
    Mut,
    Pinned,
}

impl VtableReceiver {
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

fn vtable_fn_pointer(method: &TraitItemFn, new_vtable: bool) -> TokenStream {
    let unsafety = &method.sig.unsafety;
    // We don't care about self lifetime because it is erased anyway
    let storage = match VtableReceiver::new(method) {
        VtableReceiver::Ref => quote!(&__Storage),
        VtableReceiver::Mut => quote!(&mut __Storage),
        VtableReceiver::Pinned => quote!(::core::pin::Pin<&mut __Storage>),
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
