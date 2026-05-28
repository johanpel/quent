// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Tests for the `Resource` convention validator and the
//! `ValidatorRegistry` it plugs into.

use quent_v2_model::entity::Entity as _;
use quent_v2_model_ir::{
    Model,
    convention::Convention,
    entity::Entity,
    event::{Cardinality, EntityRefRole, EntityRefTarget, Event, EventField, EventFieldType},
    identifier::Identifier,
};
use quent_v2_resource::Resource;
use quent_v2_validation::{ValidationError, ValidatorRegistry};

use source::resources::SingleOccupancy;

mod source;
mod utils;

use crate::utils::ident;

fn empty_model_with(entities: Vec<Entity>) -> Model {
    Model {
        name: ident("TestModel"),
        docs: None,
        entities,
        records: vec![],
        conventions: vec![],
    }
}

fn validate(model: &Model) -> Result<(), Vec<ValidationError>> {
    ValidatorRegistry::new().with::<Resource>().run(model)
}

/// Construct an event field with role `User("Usage")` targeting the entity
/// named `target`.
fn usage_field(target: &str) -> EventField {
    EventField {
        name: Identifier::new_unchecked("worker"),
        docs: None,
        ty: EventFieldType::EntityRef {
            role_type: EntityRefRole::User(Identifier::new_unchecked("Usage")),
            entity_type: EntityRefTarget::Specific(ident(target)),
        },
        conventions: Vec::new(),
    }
}

fn event(name: &str, payload: Vec<EventField>) -> Event {
    Event {
        name: ident(name),
        docs: None,
        cardinality: Cardinality::Once,
        payload,
        conventions: Vec::new(),
    }
}

fn entity_with(name: &str, events: Vec<Event>, conventions: Vec<Convention>) -> Entity {
    Entity {
        name: ident(name),
        docs: None,
        events,
        fsm: None,
        conventions,
    }
}

#[test]
fn usage_field_without_fsm_is_rejected() {
    let consumer = entity_with(
        "Consumer",
        vec![event("Consumer", vec![usage_field("SomePool")])],
        Vec::new(),
    );

    let model = empty_model_with(vec![consumer]);
    let result = validate(&model);
    assert!(
        result.is_err(),
        "expected validation error for non-FSM Usage field"
    );
    let errs = result.unwrap_err();
    let messages: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("Consumer") && m.contains("FSM")),
        "expected Usage-requires-FSM error, got: {messages:?}",
    );
}

#[test]
fn well_formed_resource_passes() {
    let model = empty_model_with(vec![SingleOccupancy::ir()]);
    assert_eq!(validate(&model), Ok(()));
}

fn fsm_consumer_using(target: &str) -> Entity {
    use quent_v2_model_ir::fsm::{Fsm, State, Transition};
    Entity {
        name: ident("Consumer"),
        docs: None,
        events: vec![event("A", vec![usage_field(target)])],
        fsm: Some(Fsm {
            transitions: vec![
                Transition {
                    source: State::Entry,
                    target: State::State(ident("A")),
                },
                Transition {
                    source: State::State(ident("A")),
                    target: State::Exit,
                },
            ],
        }),
        conventions: Vec::new(),
    }
}

#[test]
fn usage_target_missing_in_model_is_rejected() {
    let consumer = fsm_consumer_using("MissingPool");
    let model = empty_model_with(vec![consumer]);
    let result = validate(&model);
    assert!(
        result.is_err(),
        "expected validation error for missing Usage target"
    );
    let messages: Vec<String> = result.unwrap_err().iter().map(|e| e.to_string()).collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("MissingPool") && m.contains("not declared")),
        "expected missing-target error, got: {messages:?}",
    );
}

#[test]
fn usage_target_not_a_resource_is_rejected() {
    let non_resource = entity_with(
        "NotAResource",
        vec![event("NotAResource", vec![])],
        Vec::new(),
    );
    let consumer = fsm_consumer_using("NotAResource");
    let model = empty_model_with(vec![non_resource, consumer]);
    let result = validate(&model);
    assert!(
        result.is_err(),
        "expected validation error for non-resource Usage target"
    );
    let messages: Vec<String> = result.unwrap_err().iter().map(|e| e.to_string()).collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("NotAResource") && m.contains("not a Resource")),
        "expected not-a-Resource error, got: {messages:?}",
    );
}

#[test]
fn well_formed_consumer_of_resource_passes() {
    let consumer = fsm_consumer_using("SingleOccupancy");
    let model = empty_model_with(vec![SingleOccupancy::ir(), consumer]);
    assert_eq!(validate(&model), Ok(()));
}

#[test]
fn unregistered_convention_is_rejected() {
    // Build a model where an entity carries a Validated convention name without
    // a registered validator. The registry must flag it.
    let entity = entity_with(
        "E",
        vec![event("E", vec![])],
        vec![Convention {
            name: "BogusConvention".to_string(),
            validated: true,
            data: Some("{}".to_string()),
        }],
    );
    let model = empty_model_with(vec![entity]);
    let result = validate(&model);
    assert!(result.is_err());
    let messages: Vec<String> = result.unwrap_err().iter().map(|e| e.to_string()).collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("BogusConvention") && m.contains("unregistered")),
        "expected unregistered-convention error, got: {messages:?}",
    );
}

/// Metadata-only entries are skipped by the registry: a model with an
/// unregistered Metadata convention validates cleanly.
#[test]
fn metadata_convention_does_not_require_validator() {
    let entity = entity_with(
        "E",
        vec![event("E", vec![])],
        vec![Convention {
            name: "SomeMetadata".to_string(),
            validated: false,
            data: Some("anything".to_string()),
        }],
    );
    let model = empty_model_with(vec![entity]);
    assert_eq!(validate(&model), Ok(()));
}

/// Validated entries with no registered validator are still rejected,
/// confirming the Metadata-vs-Validated distinction.
#[test]
fn validated_convention_without_validator_is_rejected() {
    let entity = entity_with(
        "E",
        vec![event("E", vec![])],
        vec![Convention {
            name: "SomeValidated".to_string(),
            validated: true,
            data: Some("payload".to_string()),
        }],
    );
    let model = empty_model_with(vec![entity]);
    let result = validate(&model);
    assert!(result.is_err());
    let errs = result.unwrap_err();
    assert!(matches!(
        errs.as_slice(),
        [ValidationError::UnregisteredConvention { convention, .. }] if convention == "SomeValidated"
    ));
}
