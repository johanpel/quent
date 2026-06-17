// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! `model!` proc macro implementation.
//!
//! Syntax:
//! ```ignore
//! model! {
//!     Simulator {
//!         root: ResourceRoot,
//!         quent_query_engine_model::Engine,
//!         task::Task,
//!         quent_stdlib::memory::Memory,
//!     }
//! }
//! ```
//!
//! Generates `SimulatorModel` (type alias) and `SimulatorEvent` (event enum).

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Path, Token};

struct DefineModelInput {
    name: Ident,
    root: Path,
    components: Vec<Path>,
}

impl Parse for DefineModelInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;

        let content;
        syn::braced!(content in input);

        // First entry must be `root: Path`
        if content.is_empty() {
            return Err(syn::Error::new_spanned(
                name,
                "model! requires at least a root resource group: `root: MyRoot`",
            ));
        }
        let root_kw: Ident = content.parse()?;
        if root_kw != "root" {
            return Err(syn::Error::new_spanned(
                root_kw,
                "first entry must be `root: <RootResourceGroup>`",
            ));
        }
        content.parse::<Token![:]>()?;
        let root: Path = content.parse()?;
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }

        let mut components = Vec::new();
        while !content.is_empty() {
            components.push(content.parse::<Path>()?);
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        Ok(DefineModelInput {
            name,
            root,
            components,
        })
    }
}

/// Extract the last segment of a path as an Ident.
fn last_segment(path: &Path) -> Ident {
    path.segments.last().unwrap().ident.clone()
}

/// Given a path like `foo::bar::Baz`, construct `foo::bar::BazObserver`.
fn observer_type_path(path: &Path) -> Path {
    let mut obs_path = path.clone();
    if let Some(last) = obs_path.segments.last_mut() {
        last.ident = format_ident!("{}Observer", last.ident);
    }
    obs_path
}

/// Given a path like `foo::bar::Baz`, construct `foo::bar::BazEvent`.
fn event_type_path(path: &Path) -> Path {
    let mut event_path = path.clone();
    if let Some(last) = event_path.segments.last_mut() {
        last.ident = format_ident!("{}Event", last.ident);
    }
    event_path
}

/// Build a nested tuple type from a list of paths, chunking into groups of 16.
fn nested_tuple(paths: &[Path]) -> TokenStream {
    if paths.len() <= 16 {
        quote! { (#(#paths,)*) }
    } else {
        let chunks: Vec<TokenStream> = paths
            .chunks(16)
            .map(|chunk| quote! { (#(#chunk,)*) })
            .collect();
        quote! { (#(#chunks,)*) }
    }
}

pub fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let serde_derives = crate::util::serde_derives();
    let serde_crate_attr = crate::util::serde_crate_attr();
    let input: DefineModelInput = syn::parse2(input)?;
    let name = &input.name;

    let model_type = format_ident!("{}Model", name);
    let event_type = format_ident!("{}Event", name);

    let root = &input.root;

    // Root is the first component, followed by the rest
    let mut all_components = vec![input.root.clone()];
    all_components.extend(input.components.iter().cloned());
    let variants: Vec<Ident> = all_components.iter().map(last_segment).collect();

    // Validate no duplicate component names (last path segment)
    {
        let mut seen = std::collections::HashMap::new();
        for (i, variant) in variants.iter().enumerate() {
            let name_str = variant.to_string();
            if let Some(&first_idx) = seen.get(&name_str) {
                let _ = first_idx;
                return Err(syn::Error::new_spanned(
                    &all_components[i],
                    format!(
                        "duplicate component name `{name_str}` — two components resolve to the same event enum variant"
                    ),
                ));
            }
            seen.insert(name_str, i);
        }
    }

    let event_types: Vec<Path> = all_components.iter().map(event_type_path).collect();
    let observer_types: Vec<Path> = all_components.iter().map(observer_type_path).collect();
    let model_tuple = nested_tuple(&all_components);
    let context_type = format_ident!("{}Context", name);
    let quent_reexport = format_ident!("__quent_{}", crate::util::to_snake_case(name));
    let impl_macro_name = format_ident!(
        "__define_{}_instrumentation",
        crate::util::to_snake_case(name)
    );

    // One observer field per entity, named with the bare entity snake-case name.
    let observer_fields: Vec<Ident> = variants
        .iter()
        .map(|variant| format_ident!("{}", crate::util::to_snake_case(variant)))
        .collect();

    let observer_methods: Vec<TokenStream> = variants
        .iter()
        .zip(observer_types.iter())
        .zip(event_types.iter())
        .zip(observer_fields.iter())
        .map(|(((variant, obs_type), comp_event), field)| {
            let method_name = format_ident!("{}_observer", crate::util::to_snake_case(variant));
            let doc_factory = format!("Create an observer for {variant} entities.");
            quote! {
                #[doc = #doc_factory]
                pub fn #method_name(&self) -> #obs_type<#comp_event> {
                    self.#field.clone()
                }
            }
        })
        .collect();

    // Per-entity observer field declarations and their construction in `try_new`.
    let observer_field_decls: Vec<TokenStream> = observer_fields
        .iter()
        .zip(observer_types.iter())
        .zip(event_types.iter())
        .map(|((field, obs_type), comp_event)| {
            quote! { #field: #obs_type<#comp_event> }
        })
        .collect();

    let observer_inits: Vec<TokenStream> = observer_fields
        .iter()
        .zip(observer_types.iter())
        .zip(event_types.iter())
        .map(|((field, obs_type), comp_event)| {
            quote! { let #field = #obs_type::new(inner.observer::<#comp_event>()?); }
        })
        .collect();

    // Per-entity observer accessor method names, reused by the router impl.
    let observer_method_names: Vec<Ident> = variants
        .iter()
        .map(|variant| format_ident!("{}_observer", crate::util::to_snake_case(variant)))
        .collect();

    let feed_arms: Vec<TokenStream> = variants
        .iter()
        .zip(observer_method_names.iter())
        .map(|(variant, method)| {
            quote! {
                #event_type::#variant(data) => self.#method().send(
                    quent_model::Event::new(event.id, event.timestamp, data),
                ),
            }
        })
        .collect();

    let doc_model = format!("Model type alias for {name}.");
    let doc_event = format!("Events emitted by the {name} model.");
    let doc_context = format!(
        "Instrumentation context for the `{name}` model.\n\
         \n\
         This is the entry point for instrumentation. Create one with \
         [`Self::try_new()`], then call the `*_observer()` methods to get \
         observers for each model component."
    );
    let doc_try_new = format!(
        "Create a new {name} instrumentation context.\n\
         \n\
         # Arguments\n\
         * `exporter` — optional exporter configuration (e.g., ndjson, msgpack). \
         Pass `None` for a no-op context that discards events."
    );

    let output = quote! {
        #[doc = #doc_model]
        pub type #model_type = quent_model::Model<#model_tuple>;

        #[doc = #doc_event]
        #[derive(#serde_derives)]
        #serde_crate_attr
        pub enum #event_type {
            #(#variants(#event_types),)*
        }

        #(
            impl From<#event_types> for #event_type {
                fn from(e: #event_types) -> Self {
                    #event_type::#variants(e)
                }
            }
        )*

        // Records this model's package and source git so exporters can trace an
        // artifact back to the crate that defines it — including out-of-repo
        // crates, whose own `build.rs` populates `QUENT_SOURCE_*` (in-repo it
        // falls back to quent's git). `env!`/`option_env!` resolve in the crate
        // that invokes `model!`. The type path and name come from `type_name`.
        impl quent_model::build_info::ModelSource for #event_type {
            fn package() -> &'static str {
                env!("CARGO_PKG_NAME")
            }
            fn source() -> quent_model::build_info::BuildInfo {
                quent_model::build_info::source_or_quent(
                    env!("CARGO_PKG_VERSION"),
                    option_env!("QUENT_SOURCE_REMOTE"),
                    option_env!("QUENT_SOURCE_COMMIT"),
                    option_env!("QUENT_SOURCE_BRANCH"),
                    option_env!("QUENT_SOURCE_DIRTY"),
                    option_env!("QUENT_SOURCE_BUILT_AT"),
                )
            }
        }

        const _: () = {
            assert!(
                <#root as quent_model::ResourceGroup>::IS_ROOT,
                "the `root:` component must be annotated with #[resource_group(root)]"
            );
        };

        #[doc(hidden)]
        pub use quent_model as #quent_reexport;

        #[doc(hidden)]
        #[macro_export]
        macro_rules! #impl_macro_name {
            () => {
                #[doc = #doc_context]
                #[doc(alias = "context")]
                pub struct #context_type {
                    #(#observer_field_decls,)*
                    _inner: quent_model::Context<#event_type>,
                }

                impl #context_type {
                    #[doc = #doc_try_new]
                    pub fn try_new(
                        exporter: Option<quent_model::exporter::ExporterOptions>,
                    ) -> Result<Self, Box<dyn std::error::Error>> {
                        let inner = quent_model::Context::<#event_type>::try_new(exporter)?;
                        #(#observer_inits)*
                        Ok(Self {
                            #(#observer_fields,)*
                            _inner: inner,
                        })
                    }

                    /// Identity of this context, generated on construction.
                    pub fn id(&self) -> quent_model::uuid::Uuid {
                        self._inner.id()
                    }

                    #(#observer_methods)*
                }

                // Collector routing for `#context_type`. Kept as a separate
                // trait impl so the context's inherent API stays a pure
                // local-production type. Emitted unconditionally for now;
                // feature-gating it is a future refinement.
                impl quent_model::CollectorContext for #context_type {
                    type Event = #event_type;

                    fn with_source_id(
                        id: quent_model::uuid::Uuid,
                        exporter: Option<quent_model::exporter::ExporterOptions>,
                    ) -> Result<Self, Box<dyn std::error::Error>> {
                        let inner = quent_model::Context::<#event_type>::try_with_id(id, exporter)?;
                        #(#observer_inits)*
                        Ok(Self {
                            #(#observer_fields,)*
                            _inner: inner,
                        })
                    }

                    fn feed(&self, event: quent_model::Event<#event_type>) {
                        match event.data {
                            #(#feed_arms)*
                        }
                    }
                }
            };
        }
    };

    Ok(output)
}

/// Expand the `instrumentation!` proc macro.
///
/// Invokes the hidden callback macro generated by `model!`.
pub fn expand_instrumentation(input: TokenStream) -> syn::Result<TokenStream> {
    let name: Ident = syn::parse2(input)?;
    let impl_macro_name = format_ident!(
        "__define_{}_instrumentation",
        crate::util::to_snake_case(&name)
    );

    Ok(quote! {
        #impl_macro_name!();
    })
}
