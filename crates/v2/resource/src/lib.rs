// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Resource convention for the Quent v2 model.
//!
//! Defines the [`Resource`] trait, capacity bound types, the [`Usage`] role,
//! and the [`Resource`] convention validator that interprets per-entity
//! [`ir::ResourceData`] under the `"Resource"` convention key.

use std::collections::HashMap;
use std::marker::PhantomData;

use quent_v2_model::entity::Entity;
use quent_v2_model::entity_ref::{EntityRef, EntityRefRole, EntityRefRoleTarget};
use quent_v2_model_ir::entity::Entity as IrEntity;
use quent_v2_model_ir::event as ir_event;
use quent_v2_model_ir::identifier::Identifier;
use quent_v2_model_ir::{Model, fsm::State};
use quent_v2_validation::{Convention, ValidationError};

pub use quent_v2_resource_macros::resource;

pub(crate) mod ir;

pub use ir::{BoundednessData, CapacityData, CapacityKindData, ResourceData};

/// Implementation detail re-exported for use by the `resource!` macro.
#[doc(hidden)]
pub mod __private {
    use super::ResourceData;

    pub fn to_json(d: &ResourceData) -> String {
        serde_json::to_string(d).expect("ResourceData JSON serialization is infallible")
    }
}

/// Convention name used as the key in `conventions` maps for entities that
/// qualify as a resource and on event fields that represent a resource usage.
pub const CONVENTION_NAME: &str = "Resource";

/// Zero-sized type that wires resource-specific validation into the
/// [`quent_v2_validation::ValidatorRegistry`].
pub struct Resource;

impl Convention for Resource {
    const NAME: &'static str = CONVENTION_NAME;

    fn validate(model: &Model) -> Result<(), Vec<ValidationError>> {
        let mut errors: Vec<ValidationError> = Vec::new();
        let convention_id = Identifier::new_unchecked(CONVENTION_NAME);

        // Index entities by name and pre-decode their ResourceData payloads.
        let by_name: HashMap<&str, &IrEntity> = model
            .entities
            .iter()
            .map(|e| (e.name.as_str(), e))
            .collect();
        let mut resource_data: HashMap<&str, ResourceData> = HashMap::new();
        for entity in &model.entities {
            let Some(entry) = entity
                .conventions
                .iter()
                .find(|c| c.name == CONVENTION_NAME)
            else {
                continue;
            };
            let Some(raw) = entry.data.as_deref() else {
                errors.push(ValidationError::ConventionError {
                    convention: convention_id.clone(),
                    message: format!(
                        "entity '{}': Resource convention requires non-empty data",
                        entity.name
                    ),
                });
                continue;
            };
            match serde_json::from_str::<ResourceData>(raw) {
                Ok(data) => {
                    resource_data.insert(entity.name.as_str(), data);
                }
                Err(e) => errors.push(ValidationError::ConventionError {
                    convention: convention_id.clone(),
                    message: format!(
                        "entity '{}': failed to decode Resource data: {e}",
                        entity.name
                    ),
                }),
            }
        }

        for entity in &model.entities {
            // Rule 1 + 3: Usage fields require FSM parent and a valid resource target.
            for event in &entity.events {
                for field in &event.payload {
                    let ir_event::EventFieldType::EntityRef {
                        role_type,
                        entity_type,
                    } = &field.ty
                    else {
                        continue;
                    };
                    let ir_event::EntityRefRole::User(role) = role_type else {
                        continue;
                    };
                    if role.as_str() != "Usage" {
                        continue;
                    }
                    let target_name = match entity_type {
                        ir_event::EntityRefTarget::Specific(id) => id.as_str(),
                        ir_event::EntityRefTarget::Any => {
                            errors.push(ValidationError::ConventionError {
                                convention: convention_id.clone(),
                                message: format!(
                                    "entity '{}': Usage<...> field must target a specific entity",
                                    entity.name,
                                ),
                            });
                            continue;
                        }
                    };
                    if entity.fsm.is_none() {
                        errors.push(ValidationError::ConventionError {
                            convention: convention_id.clone(),
                            message: format!(
                                "entity '{}' has a Usage<...> field but declares no FSM; \
                                 resource usages must occur within a time window",
                                entity.name,
                            ),
                        });
                    }
                    match by_name.get(target_name) {
                        None => errors.push(ValidationError::ConventionError {
                            convention: convention_id.clone(),
                            message: format!(
                                "entity '{}' has a Usage<{}> field but '{}' is not declared in the model",
                                entity.name, target_name, target_name,
                            ),
                        }),
                        Some(_) if !resource_data.contains_key(target_name) => {
                            errors.push(ValidationError::ConventionError {
                                convention: convention_id.clone(),
                                message: format!(
                                    "entity '{}' has a Usage<{}> field but '{}' is not a Resource",
                                    entity.name, target_name, target_name,
                                ),
                            });
                        }
                        _ => {}
                    }
                }
            }

            // Rule 2: Resource entities must satisfy the canonical lifecycle FSM.
            let Some(rd) = resource_data.get(entity.name.as_str()) else {
                continue;
            };
            if rd.capacities.is_empty() {
                errors.push(ValidationError::ConventionError {
                    convention: convention_id.clone(),
                    message: format!(
                        "entity '{}' carries a Resource convention but declares no capacities",
                        entity.name,
                    ),
                });
            }
            let Some(fsm) = &entity.fsm else {
                errors.push(ValidationError::ConventionError {
                    convention: convention_id.clone(),
                    message: format!(
                        "entity '{}' carries a Resource convention but declares no FSM",
                        entity.name,
                    ),
                });
                continue;
            };
            let any_resizable = rd
                .capacities
                .iter()
                .any(|c| matches!(c.boundedness, BoundednessData::Resizable));
            let has_named = |name: &str| -> bool {
                fsm.transitions
                    .iter()
                    .any(|t| match (&t.source, &t.target) {
                        (State::State(n), _) | (_, State::State(n)) => n.as_str() == name,
                        _ => false,
                    })
            };
            for required in ["init", "operating", "finalizing"] {
                if !has_named(required) {
                    errors.push(ValidationError::ConventionError {
                        convention: convention_id.clone(),
                        message: format!(
                            "entity '{}' (Resource) is missing required state '{}'",
                            entity.name, required,
                        ),
                    });
                }
            }
            if any_resizable && !has_named("resizing") {
                errors.push(ValidationError::ConventionError {
                    convention: convention_id.clone(),
                    message: format!(
                        "entity '{}' (Resource) has a resizable capacity but no 'resizing' state",
                        entity.name,
                    ),
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// TODO: seal traits below

/// Trait for markers defining how a capacity's bounds are to be perceived.
pub trait Boundedness {}

/// Trait for markers defining the kind of capacity.
pub trait CapacityKind {}

/// Trait for entities that are resources.
pub trait ResourceEntity: Entity {
    type UsageType;
    type BoundsType;
}

/// Marker for fixed-size bounds of a resource capacity.
pub struct Fixed;

/// Marker for bounds of a capacity that can change while the resource is in use.
pub struct Resizeable;

/// Marker for an unbounded resource capacity.
///
/// While resource capacities are in practise always subject to physical limits,
/// this marks a capacity as one where the bounds are unknown as far as the
/// model is concerned. This can be used for resource capacities where it is
/// non-trivial to obtain or unimportant to trace the bounds.
pub struct Unbounded;

impl Boundedness for Fixed {}
impl Boundedness for Resizeable {}
impl Boundedness for Unbounded {}

/// Marker for a capacity measured as an amount held at a point in time
/// (e.g. slots in use, bytes resident).
pub struct Occupancy;

/// Marker for a capacity measured as a flow or work over time
/// (e.g. items per second, bytes per nanosecond).
pub struct Rate;

impl CapacityKind for Occupancy {}
impl CapacityKind for Rate {}

pub struct Capacity<T, K = Occupancy, B = Fixed>
where
    K: CapacityKind,
    B: Boundedness,
{
    _value_type: PhantomData<T>,
    _kind: PhantomData<K>,
    _bounded: PhantomData<B>,
}

/// A bound of an [`Occupancy`]-type resource [`Capacity`].
pub struct OccupancyBound<T> {
    pub value: T,
}

/// A bound of a [`Rate`]-type resource [`Capacity`].
pub struct RateBound<T> {
    /// The number of items in the rate bound expressed as items/nanoseconds
    pub items: T,
    /// The amount of nanoseconds in the rate bound expressed as items/nanoseconds.
    pub nanoseconds: u64,
}

impl quent_v2_model::record::Record for OccupancyBound<u64> {
    #[cfg(feature = "ir")]
    fn ir() -> quent_v2_model_ir::record::Record {
        use quent_v2_model_ir::{
            data_type::DataType, identifier::Identifier, record::Field, record::Record,
        };
        Record {
            name: Identifier::new_unchecked("OccupancyBound"),
            docs: None,
            fields: vec![Field {
                name: Identifier::new_unchecked("value"),
                docs: None,
                ty: DataType::U64,
                conventions: Vec::new(),
            }],
            conventions: Vec::new(),
        }
    }
}

impl quent_v2_model::data_type::DataType for OccupancyBound<u64> {
    #[cfg(feature = "ir")]
    fn ir() -> quent_v2_model_ir::data_type::DataType {
        quent_v2_model_ir::data_type::DataType::Record(
            quent_v2_model_ir::identifier::Identifier::new_unchecked("OccupancyBound"),
        )
    }
}

impl quent_v2_model::record::Record for RateBound<u64> {
    #[cfg(feature = "ir")]
    fn ir() -> quent_v2_model_ir::record::Record {
        use quent_v2_model_ir::{
            data_type::DataType, identifier::Identifier, record::Field, record::Record,
        };
        Record {
            name: Identifier::new_unchecked("RateBound"),
            docs: None,
            fields: vec![
                Field {
                    name: Identifier::new_unchecked("items"),
                    docs: None,
                    ty: DataType::U64,
                    conventions: Vec::new(),
                },
                Field {
                    name: Identifier::new_unchecked("nanoseconds"),
                    docs: None,
                    ty: DataType::U64,
                    conventions: Vec::new(),
                },
            ],
            conventions: Vec::new(),
        }
    }
}

impl quent_v2_model::data_type::DataType for RateBound<u64> {
    #[cfg(feature = "ir")]
    fn ir() -> quent_v2_model_ir::data_type::DataType {
        quent_v2_model_ir::data_type::DataType::Record(
            quent_v2_model_ir::identifier::Identifier::new_unchecked("RateBound"),
        )
    }
}

/// An [`crate::entity_ref::EntityRef`] role for FSMs to convey they are using a
/// resource for the duration of some state.
pub struct Usage<R>
where
    R: ResourceEntity,
{
    pub amounts: R::UsageType,
}

/// Shorthand for an [`EntityRef`] with the [`Usage`] role pointing at a
/// resource `R`. The role-target pair is fully determined by `R`, so the two
/// type parameters of `EntityRef` collapse to one.
pub type UsageRef<R> = EntityRef<Usage<R>, R>;

// Usage is a role of a reference
impl<R: ResourceEntity> EntityRefRole for Usage<R> {
    #[cfg(feature = "ir")]
    fn ir() -> ir_event::EntityRefRole {
        ir_event::EntityRefRole::User(Identifier::new_unchecked("Usage"))
    }
}
// A reference with a resource usage role can only target resource entities
impl<R: ResourceEntity> EntityRefRoleTarget<R> for Usage<R> {}

// EventField impl for Usage<R>: emits an EntityRef event field with the
// `User("Usage")` role and a target derived from R.
impl<R> quent_v2_model::event::EventField for Usage<R>
where
    R: ResourceEntity,
{
    #[cfg(feature = "ir")]
    fn ir() -> ir_event::EventFieldType {
        let target = R::ir_ref_target();
        if !matches!(target, ir_event::EntityRefTarget::Specific(_)) {
            unreachable!("resource usages can only target resource entities")
        }
        ir_event::EventFieldType::EntityRef {
            role_type: ir_event::EntityRefRole::User(Identifier::new_unchecked("Usage")),
            entity_type: target,
        }
    }
}
