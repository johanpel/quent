use convert_case::{Case, Casing};
use indexmap::IndexMap;
use proc_macro2::TokenStream;
use quent_v2_model_ir::qualifications::fsm::{Fsm, State, Transition};
use quote::{format_ident, quote};
use syn::Variant;

pub(crate) fn emit_observer(
    name: &syn::Ident,
    observer_name: &syn::Ident,
    handle_name: &syn::Ident,
    vis: &syn::Visibility,
    variants: &IndexMap<String, &Variant>,
    fsm: &Fsm,
) -> syn::Result<TokenStream> {
    // Safety: if entity validation passes, unwraps in this fn will not panic.

    let state_mod_name = format_ident!("{}_state", name.to_string().to_case(Case::Snake));
    let state_initial_str = fsm.initial_state().unwrap().as_str();
    let state_initial_ident = format_ident!("{}", state_initial_str);
    let obs_handle_construct_ident = format_ident!("{}", state_initial_str.to_case(Case::Snake));

    let variant = variants.get(state_initial_str).unwrap();
    let (transition_method_args, transition_event_payload) =
        transition_args_payload(name, variant)?;

    Ok(quote! {
        #vis struct #observer_name {
            inner: ::quent_v2_instrumentation::Observer<
                ::quent_v2_instrumentation::handle::fsm::Transition<#name>
            >,
        }

        impl #observer_name {
            #vis fn try_new(
                root_id: ::uuid::Uuid,
                opts: ::std::option::Option<::quent_v2_instrumentation::ExporterOptions>,
            ) -> ::std::result::Result<Self, ::quent_v2_instrumentation::ObserverError> {
                ::std::result::Result::Ok(Self {
                    inner: ::quent_v2_instrumentation::Observer::new(root_id, opts)?,
                })
            }

            #vis fn #obs_handle_construct_ident(
                &self,
                #transition_method_args
            ) -> ::std::result::Result<#handle_name<#state_mod_name::#state_initial_ident>, ::quent_v2_instrumentation::ObserverError> {
                let inner = ::quent_v2_instrumentation::handle::fsm::FsmHandle::new(
                    self.inner.sender(),
                    ::uuid::Uuid::now_v7(),
                );
                inner.emit_normal(#transition_event_payload)?;
                ::std::result::Result::Ok(#handle_name {
                    inner,
                    _state: ::std::marker::PhantomData,
                })
            }
        }
    })
}

pub(crate) fn emit_handle(
    ident: &syn::Ident,
    handle_name: &syn::Ident,
    vis: &syn::Visibility,
    variants: &IndexMap<String, &Variant>,
    fsm: &Fsm,
) -> syn::Result<TokenStream> {
    // Safety: if entity validation passes, unwraps and unreachable in this fn will not panic.

    let state_mod_name = format_ident!("{}_state", ident.to_string().to_case(Case::Snake));
    let state_marker_idents: Vec<&syn::Ident> = variants.iter().map(|(_, v)| &v.ident).collect();

    // TODO(johanpel): sealed trait for markers
    let state_mod = quote! {
        #vis mod #state_mod_name {
            #(pub struct #state_marker_idents;)*
        }
    };

    let handle_type = quote! {
        #vis struct #handle_name<S> {
            inner: ::quent_v2_instrumentation::handle::fsm::FsmHandle<#ident>,
            _state: ::std::marker::PhantomData<S>,
        }
    };

    let mut transitions: IndexMap<&str, Vec<&Transition>> =
        variants.iter().map(|(v, _)| (v.as_str(), vec![])).collect();
    for t in &fsm.transitions {
        if let State::State(src_name) = &t.source {
            transitions.get_mut(src_name.as_str()).unwrap().push(t);
        }
    }

    // Generate all the typestate pattern impls for the handle
    let handle_typestate_impls: Vec<TokenStream> = transitions
        .into_iter()
        .map(|(state_source_str, transitions)| -> syn::Result<TokenStream> {
            let state_source_ident = format_ident!("{}", state_source_str);
            let transition_methods: Vec<TokenStream> = transitions
                .iter()
                .map(|t| -> syn::Result<TokenStream> {
                    match &t.target {
                        State::Exit => Ok(quote! {
                            #vis fn exit(self) -> ::std::result::Result<
                                ::uuid::Uuid,
                                ::quent_v2_instrumentation::ObserverError,
                            > {
                                self.inner.emit_exit()?;
                                Ok(self.inner.id())
                            }
                        }),
                        State::State(state_target) => {
                            let state_target_str = state_target.as_str();
                            let state_target_ident = format_ident!("{}", state_target_str);
                            let transition_method_ident = format_ident!("{}", state_target_str.to_case(Case::Snake));
                            let state_variant = variants.get(state_target_str).unwrap();
                            let (transition_method_args, transition_event_payload) = transition_args_payload(ident, state_variant)?;
                            Ok(quote! {
                                #vis fn #transition_method_ident(
                                    self,
                                    #transition_method_args
                                ) -> ::std::result::Result<#handle_name<#state_mod_name::#state_target_ident>, ::quent_v2_instrumentation::ObserverError> {
                                    self.inner.emit_normal(#transition_event_payload)?;
                                    ::std::result::Result::Ok(#handle_name {
                                        inner: self.inner,
                                        _state: ::std::marker::PhantomData,
                                    })
                                }
                            })
                        }
                        State::Entry => unreachable!(),
                    }
                })
                .collect::<syn::Result<_>>()?;

            Ok(quote! {
                impl #handle_name<#state_mod_name::#state_source_ident> {
                    #(#transition_methods)*
                }
            })
        })
        .collect::<syn::Result<_>>()?;

    Ok(quote! {
        #state_mod

        #handle_type

        #(#handle_typestate_impls)*
    })
}

fn transition_args_payload(
    enum_name: &syn::Ident,
    enum_variant: &Variant,
) -> syn::Result<(TokenStream, TokenStream)> {
    // Safety: if entity validation passes, unwraps and unreachable in this fn will not panic.

    let variant_ident = &enum_variant.ident;
    match &enum_variant.fields {
        syn::Fields::Unit => Ok((quote! {}, quote! { #enum_name::#variant_ident })),
        syn::Fields::Unnamed(u) if u.unnamed.len() == 1 => {
            let ty = &u.unnamed.first().unwrap().ty;
            Ok((
                quote! { payload: #ty },
                quote! { #enum_name::#variant_ident(payload) },
            ))
        }
        syn::Fields::Unnamed(_) => unreachable!(),
        syn::Fields::Named(named) => {
            let arg_defs: Vec<TokenStream> = named
                .named
                .iter()
                .map(|f| {
                    let ident = f.ident.as_ref().unwrap();
                    let ty = &f.ty;
                    quote! { #ident: #ty }
                })
                .collect();
            let field_idents: Vec<_> = named
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap())
                .collect();
            Ok((
                quote! { #(#arg_defs),* },
                quote! { #enum_name::#variant_ident { #(#field_idents),* } },
            ))
        }
    }
}
