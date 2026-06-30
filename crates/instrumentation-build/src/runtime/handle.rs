// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of per-entity handles — the per-instance emit surface.

use convert_case::Case;
use proc_macro2::{Literal, TokenStream};
use quent_schema::{Cardinality, Entity};
use quote::quote;

use super::{event_ident, handle_ident};
use crate::GenerateError;
use crate::common::{doc_attr_or, raw_ident, to_case};
use crate::data_type::map_data_type;

/// The maximum once-events an entity may declare: one bit per event in the
/// handle's `u64` once-flag word.
pub(crate) const MAX_ONCE_EVENTS: usize = u64::BITS as usize;

/// The `{Entity}Handle`: one emit method per event over a runtime `Handle`.
/// Once-events take `&mut self` and are guarded by a flag bit; multi-events
/// take `&self`.
///
/// # Errors
///
/// Returns [`GenerateError::TooManyOnceEvents`] if the entity declares more
/// once-cardinality events than fit the once-flag word.
pub(super) fn entity_handle(entity: &Entity) -> Result<TokenStream, GenerateError> {
    let entity_pascal = to_case(entity.name(), Case::Pascal);
    let event_ty = event_ident(entity);
    let handle_ty = handle_ident(entity);

    let once_count = entity
        .events()
        .filter(|e| e.cardinality() == Cardinality::Once)
        .count();
    if once_count > MAX_ONCE_EVENTS {
        return Err(GenerateError::TooManyOnceEvents {
            entity: entity.name().clone(),
            count: once_count,
        });
    }

    // Once-events claim successive bits of the handle's flag word, in
    // declaration order; multi-events route straight through `emit`.
    let mut once_bit = 0u32;
    let methods: Vec<TokenStream> = entity
        .events()
        .map(|event| {
            let method = raw_ident(to_case(event.name(), Case::Snake));
            let variant = raw_ident(to_case(event.name(), Case::Pascal));
            let fallback = match event.cardinality() {
                Cardinality::Once => format!(
                    "Emit the once-cardinality `{}` event for this instance.",
                    event.name()
                ),
                Cardinality::Multi => {
                    format!("Emit a `{}` event for this instance.", event.name())
                }
            };
            let docs = doc_attr_or(event.annotations().docs(), &fallback);

            let params: Vec<TokenStream> = event
                .fields()
                .map(|f| {
                    let name = raw_ident(to_case(f.name(), Case::Snake));
                    let ty = map_data_type(f.ty(), 0);
                    quote! { #name: #ty }
                })
                .collect();
            let field_names: Vec<TokenStream> = event
                .fields()
                .map(|f| {
                    let name = raw_ident(to_case(f.name(), Case::Snake));
                    quote! { #name }
                })
                .collect();
            let construct = if field_names.is_empty() {
                quote! { #event_ty::#variant }
            } else {
                quote! { #event_ty::#variant { #(#field_names),* } }
            };

            match event.cardinality() {
                Cardinality::Once => {
                    let bit = Literal::u32_unsuffixed(once_bit);
                    once_bit += 1;
                    let event_name = event.name().to_string();
                    quote! {
                        #docs
                        pub fn #method(
                            &mut self,
                            #(#params),*
                        ) -> ::core::result::Result<(), ::quent_instrumentation::ObserverError> {
                            self.inner.emit_once(#bit, #event_name, #construct)
                        }
                    }
                }
                Cardinality::Multi => quote! {
                    #docs
                    pub fn #method(
                        &self,
                        #(#params),*
                    ) -> ::core::result::Result<(), ::quent_instrumentation::ObserverError> {
                        self.inner.emit(#construct);
                        ::core::result::Result::Ok(())
                    }
                },
            }
        })
        .collect();

    let handle_doc = format!("Handle to one `{entity_pascal}` entity instance.");
    Ok(quote! {
        #[doc = #handle_doc]
        pub struct #handle_ty {
            inner: ::quent_instrumentation::Handle<#event_ty>,
        }

        impl #handle_ty {
            /// Id of the entity instance this handle emits for.
            pub fn uuid(&self) -> ::quent_instrumentation::Uuid {
                self.inner.id()
            }

            #(#methods)*
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::pretty;
    use quent_schema::DataType;
    use quent_schema::builder::{EntityBuilder, EventBuilder};
    use quent_schema::test_utils::{field, ident};

    fn once(
        name: &str,
        fields: impl IntoIterator<Item = quent_schema::Field>,
    ) -> quent_schema::Event {
        EventBuilder::new(ident(name), Cardinality::Once)
            .fields(fields)
            .unwrap()
            .build()
    }

    fn multi(
        name: &str,
        fields: impl IntoIterator<Item = quent_schema::Field>,
    ) -> quent_schema::Event {
        EventBuilder::new(ident(name), Cardinality::Multi)
            .fields(fields)
            .unwrap()
            .build()
    }

    fn entity(name: &str, events: impl IntoIterator<Item = quent_schema::Event>) -> Entity {
        EntityBuilder::new(ident(name))
            .events(events)
            .unwrap()
            .build()
    }

    #[test]
    fn once_takes_mut_self_and_multi_takes_ref() {
        let e = entity(
            "Connection",
            [
                once(
                    "opened",
                    [
                        field("peer", DataType::String),
                        field("port", DataType::U16),
                    ],
                ),
                multi("data", [field("bytes", DataType::U64)]),
                once("closed", []),
            ],
        );
        let expected = quote! {
            #[doc = "Handle to one `Connection` entity instance."]
            pub struct ConnectionHandle {
                inner: ::quent_instrumentation::Handle<ConnectionEvent>,
            }
            impl ConnectionHandle {
                /// Id of the entity instance this handle emits for.
                pub fn uuid(&self) -> ::quent_instrumentation::Uuid {
                    self.inner.id()
                }
                #[doc = "Emit the once-cardinality `opened` event for this instance."]
                pub fn opened(
                    &mut self,
                    peer: String,
                    port: u16,
                ) -> ::core::result::Result<(), ::quent_instrumentation::ObserverError> {
                    self.inner.emit_once(0, "opened", ConnectionEvent::Opened { peer, port })
                }
                #[doc = "Emit a `data` event for this instance."]
                pub fn data(
                    &self,
                    bytes: u64,
                ) -> ::core::result::Result<(), ::quent_instrumentation::ObserverError> {
                    self.inner.emit(ConnectionEvent::Data { bytes });
                    ::core::result::Result::Ok(())
                }
                #[doc = "Emit the once-cardinality `closed` event for this instance."]
                pub fn closed(
                    &mut self,
                ) -> ::core::result::Result<(), ::quent_instrumentation::ObserverError> {
                    self.inner.emit_once(1, "closed", ConnectionEvent::Closed)
                }
            }
        };
        assert_eq!(pretty(entity_handle(&e).unwrap()), pretty(expected));
    }

    #[test]
    fn once_events_claim_successive_bits() {
        let e = entity(
            "Job",
            [once("started", []), multi("tick", []), once("finished", [])],
        );
        let src = pretty(entity_handle(&e).unwrap());
        assert!(src.contains(r#"emit_once(0, "started", JobEvent::Started)"#));
        assert!(src.contains(r#"emit_once(1, "finished", JobEvent::Finished)"#));
    }

    #[test]
    fn too_many_once_events_is_an_error() {
        let events = (0..=MAX_ONCE_EVENTS).map(|i| once(&format!("e{i}"), []));
        let e = entity("Big", events);
        let err = entity_handle(&e).unwrap_err();
        assert!(matches!(
            err,
            GenerateError::TooManyOnceEvents { count, .. } if count == MAX_ONCE_EVENTS + 1
        ));
    }
}
