// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generation of consuming typestate handles for FSM entities.

use std::collections::HashSet;

use convert_case::Case;
use proc_macro2::TokenStream;
use quent_constraints::Constraint as _;
use quent_fsm::{Fsm, SEQUENCE_FIELD_NAME};
use quent_schema::{Entity, Event, Schema};
use quote::quote;

use super::{GeneratedHandle, event_construct, event_fields, event_params};
use crate::common::{doc_attr_or, raw_ident, relative_root_type, to_case};
use crate::runtime::{event_ident, marker_ident, model_ident};
use crate::{GenerateError, Options};

pub(super) fn entity_handle(
    entity: &Entity,
    opts: &Options,
) -> Result<Option<GeneratedHandle>, GenerateError> {
    let Some(fsm) = Fsm::try_from_entity(entity)? else {
        return Ok(None);
    };
    let handle_ty = relative_root_type("FsmHandle", entity.path().namespace());
    Ok(Some(GeneratedHandle {
        tokens: generate(entity, &fsm, opts)?,
        associated_type: quote! { #handle_ty<Self> },
    }))
}

pub(super) fn handle_type(schema: &Schema) -> TokenStream {
    if !schema.entities().any(|entity| {
        entity
            .annotations()
            .has_constraint(quent_fsm::FsmConstraint::NAME)
    }) {
        return TokenStream::new();
    }
    let model = model_ident(schema);
    let docs = format!(
        "Handle to one FSM entity instance in the `{}` instrumentation model.",
        schema.name()
    );
    quote! {
        #[doc = #docs]
        pub struct FsmHandle<
            E: ::quent_instrumentation::InstrumentedEntity<Context = Context<#model>>,
            S = (),
        > {
            inner: ::quent_instrumentation::FsmHandleInner<E>,
            _state: ::core::marker::PhantomData<S>,
        }

        impl<E: ::quent_instrumentation::InstrumentedEntity<Context = Context<#model>>>
            ::core::convert::From<::quent_instrumentation::HandleInner<E>> for FsmHandle<E>
        {
            fn from(inner: ::quent_instrumentation::HandleInner<E>) -> Self {
                Self {
                    inner: inner.into(),
                    _state: ::core::marker::PhantomData,
                }
            }
        }

        impl<
            E: ::quent_instrumentation::InstrumentedEntity<Context = Context<#model>>,
            S,
        > FsmHandle<E, S>
        {
            /// Returns the entity instance ID.
            pub fn uuid(&self) -> ::quent_instrumentation::Uuid {
                self.inner.uuid()
            }

            /// Returns a typed reference to this instance carrying no data.
            pub fn as_entity_ref(&self) -> ::quent_instrumentation::EntityRef<E> {
                self.inner.as_entity_ref()
            }

            /// Returns a typed reference to this instance carrying `data`.
            pub fn as_entity_ref_with<T>(&self, data: T) -> ::quent_instrumentation::EntityRef<E, T> {
                self.inner.as_entity_ref_with(data)
            }

            /// Returns an untyped reference to this instance carrying no data.
            pub fn as_any_entity_ref(&self) -> ::quent_instrumentation::EntityRef<::quent_instrumentation::AnyEntity> {
                self.inner.as_any_entity_ref()
            }

            /// Returns an untyped reference to this instance carrying `data`.
            pub fn as_any_entity_ref_with<T>(
                &self,
                data: T,
            ) -> ::quent_instrumentation::EntityRef<::quent_instrumentation::AnyEntity, T> {
                self.inner.as_any_entity_ref_with(data)
            }
        }
    }
}

fn generate(entity: &Entity, fsm: &Fsm, opts: &Options) -> Result<TokenStream, GenerateError> {
    let event_ty = event_ident(entity);
    let marker_ty = marker_ident(entity);
    let handle_ty = relative_root_type("FsmHandle", entity.path().namespace());
    let state_module = raw_ident(format!(
        "{}_state",
        to_case(entity.path().name(), Case::Snake)
    ));
    let state_module_doc = format!("Typestate markers for the `{}` FSM.", entity.path());

    let markers = entity.events().map(|event| {
        let marker = raw_ident(to_case(event.name(), Case::Pascal));
        let marker_doc = format!(
            "Handle state after the FSM transitions into `{}`.",
            event.name()
        );
        quote! {
            #[doc = #marker_doc]
            pub enum #marker {}
        }
    });

    let sequence_arms = entity.events().map(|event| {
        let variant = raw_ident(to_case(event.name(), Case::Pascal));
        quote! { Self::#variant { seq, .. } => *seq = sequence }
    });

    let initial_event = entity
        .events()
        .find(|event| event.name() == fsm.initial_state())
        .expect("validated FSM initial state has an event");
    let initial_method = transition_method(
        entity,
        initial_event,
        &event_ty,
        &marker_ty,
        &handle_ty,
        &state_module,
        opts,
    )?;

    let mut emitted_edges = HashSet::new();
    let mut methods_by_source = Vec::new();
    for source_event in entity.events() {
        let methods = fsm
            .transitions()
            .iter()
            .filter(|transition| transition.source() == source_event.name())
            .filter(|transition| {
                emitted_edges.insert((transition.source().clone(), transition.target().clone()))
            })
            .map(|transition| {
                let target_event = entity
                    .events()
                    .find(|event| event.name() == transition.target())
                    .expect("validated FSM transition target has an event");
                transition_method(
                    entity,
                    target_event,
                    &event_ty,
                    &marker_ty,
                    &handle_ty,
                    &state_module,
                    opts,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if methods.is_empty() {
            continue;
        }
        let source_marker = raw_ident(to_case(source_event.name(), Case::Pascal));
        methods_by_source.push(quote! {
            impl #handle_ty<#marker_ty, #state_module::#source_marker> {
                #(#methods)*
            }
        });
    }

    Ok(quote! {
        #[doc = #state_module_doc]
        pub mod #state_module {
            #(#markers)*
        }

        impl ::quent_instrumentation::FsmEvent for #event_ty {
            fn set_sequence(&mut self, sequence: u16) {
                match self {
                    #(#sequence_arms),*
                }
            }
        }

        impl #handle_ty<#marker_ty> {
            #initial_method
        }

        #(#methods_by_source)*
    })
}

#[allow(clippy::too_many_arguments)]
fn transition_method(
    entity: &Entity,
    event: &Event,
    event_ty: &syn::Ident,
    entity_ty: &syn::Ident,
    handle_ty: &TokenStream,
    state_module: &syn::Ident,
    opts: &Options,
) -> Result<TokenStream, GenerateError> {
    let method = raw_ident(to_case(event.name(), Case::Snake));
    let variant = raw_ident(to_case(event.name(), Case::Pascal));
    let target_marker = raw_ident(to_case(event.name(), Case::Pascal));
    let docs = doc_attr_or(
        event.annotations().docs(),
        &format!("Transition this FSM into `{}`.", event.name()),
    );
    let params = event_params(entity, event, opts, Some(SEQUENCE_FIELD_NAME))?;
    let sequence_placeholder = quote! { 0 };
    let fields = event_fields(event, Some((SEQUENCE_FIELD_NAME, &sequence_placeholder)));
    let construct = event_construct(event_ty, &variant, &fields);

    Ok(quote! {
        #docs
        pub fn #method(self, #(#params),*) -> #handle_ty<#entity_ty, #state_module::#target_marker> {
            #handle_ty {
                inner: self.inner.transition(#construct),
                _state: ::core::marker::PhantomData,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use quent_fsm::{FsmEntityBuilder, StateDecl};
    use quent_schema::{DataType, Field, Identifier};

    use super::*;
    use crate::common::pretty;
    use crate::runtime::handle::MAX_ONCE_EVENTS;

    fn state(name: &str, to: &[&str], initial: bool, fields: Vec<Field>) -> StateDecl {
        StateDecl {
            name: Identifier::try_new(name).unwrap(),
            attributes: fields,
            to: to
                .iter()
                .map(|target| Identifier::try_new(*target).unwrap())
                .collect(),
            initial,
        }
    }

    fn source(entity: &Entity) -> String {
        pretty(
            entity_handle(entity, &Options::default())
                .unwrap()
                .unwrap()
                .tokens,
        )
    }

    #[test]
    fn generates_consuming_linear_transitions_without_sequence_parameters() {
        let entity = FsmEntityBuilder::new("Query".parse::<quent_schema::Path>().unwrap())
            .with_states([
                state(
                    "submitted",
                    &["running"],
                    true,
                    vec![Field::new(
                        Identifier::try_new("text").unwrap(),
                        DataType::String,
                        Default::default(),
                    )],
                ),
                state("running", &["ready"], false, vec![]),
                state("ready", &[], false, vec![]),
            ])
            .build()
            .unwrap();

        let source = source(&entity);

        assert!(source.contains("pub mod query_state"));
        assert!(source.contains("pub enum Submitted"));
        assert!(source.contains("impl FsmHandle<Query>"));
        assert!(source.contains("pub fn submitted(self, text: String)"));
        assert!(source.contains("-> FsmHandle<Query, query_state::Submitted>"));
        assert!(source.contains("impl FsmHandle<Query, query_state::Submitted>"));
        assert!(source.contains(".transition(QueryEvent::Submitted"));
        assert!(source.contains("seq: 0"));
        assert!(!source.contains("seq: u16"));
        assert!(!source.contains("impl FsmHandle<Query, query_state::Ready>"));
    }

    #[test]
    fn generates_branching_cycles_self_transitions_and_namespaced_states() {
        let entity = FsmEntityBuilder::new("jobs::Query".parse::<quent_schema::Path>().unwrap())
            .with_states([
                state("submitted", &["running", "failed"], true, vec![]),
                state(
                    "running",
                    &["running", "running", "ready", "failed"],
                    false,
                    vec![],
                ),
                state("ready", &[], false, vec![]),
                state("failed", &[], false, vec![]),
            ])
            .build()
            .unwrap();

        let source = source(&entity);

        assert!(source.contains("impl super::FsmHandle<Query>"));
        assert!(source.contains("impl super::FsmHandle<Query, query_state::Running>"));
        assert!(source.contains("-> super::FsmHandle<Query, query_state::Running>"));
        assert_eq!(source.matches("pub fn running(").count(), 2);
        assert!(source.contains("pub fn ready("));
        assert!(source.contains("pub fn failed("));
        assert!(!source.contains("impl super::FsmHandle<Query, query_state::Ready>"));
        assert!(!source.contains("impl super::FsmHandle<Query, query_state::Failed>"));
    }

    #[test]
    fn fsm_entities_bypass_the_once_event_limit() {
        let states = (0..=MAX_ONCE_EVENTS)
            .map(|index| {
                let name = format!("state_{index}");
                let next = (index < MAX_ONCE_EVENTS).then(|| format!("state_{}", index + 1));
                StateDecl {
                    name: Identifier::try_new(name).unwrap(),
                    attributes: vec![],
                    to: next
                        .into_iter()
                        .map(|target| Identifier::try_new(target).unwrap())
                        .collect(),
                    initial: index == 0,
                }
            })
            .collect::<Vec<_>>();
        let entity = FsmEntityBuilder::new("Large".parse::<quent_schema::Path>().unwrap())
            .with_states(states)
            .build()
            .unwrap();

        assert!(matches!(
            entity_handle(&entity, &Options::default()),
            Ok(Some(_))
        ));
    }
}
