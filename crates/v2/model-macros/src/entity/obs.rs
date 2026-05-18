use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Token, Variant, punctuated::Punctuated};

pub fn expand_struct(
    name: &syn::Ident,
    name_str: &str,
    fields: &syn::Fields,
    input: &DeriveInput,
) -> syn::Result<TokenStream> {
    if matches!(fields, syn::Fields::Unnamed(_)) {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] on a struct requires a unit struct or a struct with named fields",
        ));
    }

    let vis = &input.vis;
    let observer_name = format_ident!("{}Observer", name);
    let handle_name = format_ident!("{}Handle", name);
    let method_name = format_ident!("{}", name_str.to_case(Case::Snake));

    let (event_arg, event_value) = match fields {
        syn::Fields::Unit => (quote! {}, quote! { #name }),
        syn::Fields::Named(n) if n.named.is_empty() => (quote! {}, quote! { #name {} }),
        _ => (quote! { payload: #name, }, quote! { payload }),
    };

    Ok(quote! {
        #vis struct #observer_name {
            inner: ::quent_v2_instrumentation::Observer<#name>,
        }

        impl #observer_name {
            #vis fn new(
                root_id: ::uuid::Uuid,
                opts: ::std::option::Option<::quent_v2_instrumentation::ExporterOptions>,
            ) -> ::std::result::Result<Self, ::quent_v2_instrumentation::ObserverError> {
                ::std::result::Result::Ok(Self {
                    inner: ::quent_v2_instrumentation::Observer::new(root_id, opts)?,
                })
            }

            #vis fn handle(&self) -> #handle_name {
                #handle_name {
                    inner: ::quent_v2_instrumentation::Handle::new(
                        self.inner.sender(),
                        ::uuid::Uuid::now_v7(),
                    ),
                }
            }
        }

        #vis struct #handle_name {
            inner: ::quent_v2_instrumentation::Handle<#name>,
        }

        impl #handle_name {
            #vis fn #method_name(
                &self,
                #event_arg
            ) -> ::std::result::Result<(), ::quent_v2_instrumentation::ObserverError> {
                self.inner.emit(#event_value)
            }
        }
    })
}

pub fn expand_enum(
    name: &syn::Ident,
    _name_str: &str,
    variants: &Punctuated<Variant, Token![,]>,
    input: &DeriveInput,
) -> syn::Result<TokenStream> {
    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "#[derive(Entity)] requires the enum to have at least one variant, otherwise it would represent an entity without an event",
        ));
    }

    let vis = &input.vis;
    let observer_name = format_ident!("{}Observer", name);
    let handle_name = format_ident!("{}Handle", name);

    let methods: Vec<TokenStream> = variants
        .iter()
        .map(|v| expand_enum_variant_to_handle_method(name, v, vis))
        .collect::<syn::Result<_>>()?;

    Ok(quote! {
        #vis struct #observer_name {
            inner: ::quent_v2_instrumentation::Observer<#name>,
        }

        impl #observer_name {
            #vis fn new(
                root_id: ::uuid::Uuid,
                opts: ::std::option::Option<::quent_v2_instrumentation::ExporterOptions>,
            ) -> ::std::result::Result<Self, ::quent_v2_instrumentation::ObserverError> {
                ::std::result::Result::Ok(Self {
                    inner: ::quent_v2_instrumentation::Observer::new(root_id, opts)?,
                })
            }

            #vis fn handle(&self) -> #handle_name {
                #handle_name {
                    inner: ::quent_v2_instrumentation::Handle::new(
                        self.inner.sender(),
                        ::uuid::Uuid::now_v7(),
                    ),
                }
            }
        }

        #vis struct #handle_name {
            inner: ::quent_v2_instrumentation::Handle<#name>,
        }

        impl #handle_name {
            #(#methods)*
        }
    })
}

fn expand_enum_variant_to_handle_method(
    enum_name: &syn::Ident,
    variant: &Variant,
    vis: &syn::Visibility,
) -> syn::Result<TokenStream> {
    let variant_ident = &variant.ident;
    let method_name = format_ident!("{}", variant_ident.to_string().to_case(Case::Snake));

    let (args, construct) = match &variant.fields {
        syn::Fields::Unit => (quote! {}, quote! { #enum_name::#variant_ident }),
        syn::Fields::Unnamed(u) if u.unnamed.len() == 1 => {
            let ty = &u.unnamed.first().unwrap().ty;
            (
                quote! { payload: #ty },
                quote! { #enum_name::#variant_ident(payload) },
            )
        }
        syn::Fields::Unnamed(_) => {
            return Err(syn::Error::new_spanned(
                variant,
                "#[derive(Entity)] does not support enum variants with more than one unnamed field",
            ));
        }
        syn::Fields::Named(named) => {
            let arg_defs = named.named.iter().map(|f| {
                let ident = f.ident.as_ref().unwrap();
                let ty = &f.ty;
                quote! { #ident: #ty }
            });
            let field_idents: Vec<_> = named
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap())
                .collect();
            (
                quote! { #(#arg_defs),* },
                quote! { #enum_name::#variant_ident { #(#field_idents),* } },
            )
        }
    };

    Ok(quote! {
        #vis fn #method_name(
            &self,
            #args
        ) -> ::std::result::Result<(), ::quent_v2_instrumentation::ObserverError> {
            self.inner.emit(#construct)
        }
    })
}
