// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The Quent built-in resource constraint.

use quent_constraints::{Constraint, utils::bullet_list};
use quent_fsm::FsmConstraint;
use quent_schema::{
    Annotations, DataType, Entity, Identifier,
    visitor::{Cursor, Element, Visitor},
};
use rustc_hash::{FxHashMap as Map, FxHashSet};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod builder;

pub use builder::{BuildError, ResourceBuilder, ResourceParts};

/// A Resource is an Entity with certain Capacities that other Entities may
/// claim through a Usage over some span of time.
///
/// Only states of FSM-type entities provide the guarantee that Usages end
/// (either by transitioning to some next state or the mandatory special exit
/// state which inherently does not hold attributes), thus only FSM-type
/// entities can use resources.
///
/// ## Requirements
///
/// 1. A resource is an entity with at least one [`Capacity`].
/// 2. The [`Identifier`] of a [`Capacity`] is unique within a resource.
/// 3. If and only if any of the resource's capacities have a bound, the
///    resource entity has at least one event (the "bounds event") which
///    declares the bounds of all capacities that are bounded.
/// 4. An entity can use some quantity of a resource's capacities if and
///    only if it is an FSM.
/// 5. The resource named by a usage or bounds is a declared resource.
/// 6. A usage claims only capacities declared by its resource.
/// 7. A usage record is used only as the data carried by an entity reference.
/// 8. A bounds record is used only by events of the resource it names.
#[derive(Default)]
pub struct ResourceConstraint {
    errors: Vec<ResourceError>,
    /// Declared resources, each mapping its capacity names to whether they are bounded.
    resources: Map<Identifier, Map<Identifier, bool>>,
    /// Usage records, by record name.
    usage_records: Map<Identifier, UsageRecord>,
    /// Bounds records, by record name.
    bounds_records: Map<Identifier, BoundsRecord>,
    /// Every record reference, for the usage- and bounds-placement checks.
    record_refs: Vec<RecordRef>,
}

/// A named, quantified dimension of a resource that usages claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capacity {
    /// The unique name of the capacity within the resource.
    name: Identifier,
    /// The type of capacity.
    kind: CapacityKind,
    /// Whether the capacity is bounded. If all capacities of a resource are
    /// unbounded, then no bounds need to be set, so no bound record type should
    /// exist, and the FSM transition into "operating" shall not have a bounds
    /// argument.
    bounded: bool,
}

impl Capacity {
    pub fn new(name: Identifier, kind: CapacityKind, bounded: bool) -> Self {
        Self {
            name,
            kind,
            bounded,
        }
    }

    pub fn name(&self) -> &Identifier {
        &self.name
    }

    pub fn kind(&self) -> CapacityKind {
        self.kind
    }

    pub fn bounded(&self) -> bool {
        self.bounded
    }
}

/// How a capacity relates to the span over which it is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapacityKind {
    /// A quantity held for the duration of a usage span, e.g. bytes of a
    /// memory.
    Occupancy,
    /// A total quantity processed over a usage span, e.g. bytes sent over a
    /// channel. Dividing it by the span's duration yields the **perceived**
    /// rate.
    Rate,
}

type Capacities = indexmap::IndexMap<Identifier, Capacity>;

/// The data a `quent.resource.v0.1.0` constraint carries.
///
/// A resource is an entity with one or more capacities that other entities can claim.
///
/// This data is placed on several schema elements. Each variant explains the
/// role of the annotated element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    /// Placed on a resource entity, declaring it a resource providing
    /// `capacities`.
    Definition(Capacities),
    /// Placed on the record type conveying the bounds of resource `resource`.
    ///
    /// This record is used by a resource's own events.
    Bounds { resource: Identifier },
    /// Placed on the record type conveying a usage of resource `resource`.
    ///
    /// The usage is perceived as held for the duration of the FSM state of the
    /// FSM entity claiming it. This record type can only be used on FSM state
    /// transition events besides exit to ensure the usage is released.
    Usage { resource: Identifier },
}

impl Resource {
    /// The constraint name under which the data is carried.
    pub const NAME: &'static str = "quent.resource.v0.1.0";

    /// The role name, for diagnostics.
    fn kind(&self) -> &'static str {
        match self {
            Resource::Definition(_) => "definition",
            Resource::Usage { .. } => "usage",
            Resource::Bounds { .. } => "bounds",
        }
    }
}

/// A usage record's named resource, claimed capacities, and where it was found.
struct UsageRecord {
    resource: Identifier,
    claims: Vec<Identifier>,
    location: String,
}

/// A bounds record's named resource, declared bounds, and where it was found.
struct BoundsRecord {
    resource: Identifier,
    fields: Vec<Identifier>,
    location: String,
}

/// A reference to a record type from somewhere in the schema.
struct RecordRef {
    record: Identifier,
    /// Whether the reference is the data carried by an entity reference.
    on_entity_ref: bool,
    /// The enclosing entity's name and whether it is an FSM, if any.
    entity: Option<(Identifier, bool)>,
    location: String,
}

impl Visitor for ResourceConstraint {
    type Output = Result<(), ResourceError>;

    fn visit(&mut self, cursor: &Cursor) {
        match cursor.current() {
            Element::Entity(entity) => match self.role(cursor, entity.annotations()) {
                // A definition declares the resource entity itself.
                Some(Resource::Definition(capacities)) => {
                    self.check_definition(entity.name(), &capacities);
                    self.resources.insert(
                        entity.name().clone(),
                        capacities
                            .values()
                            .map(|capacity| (capacity.name().clone(), capacity.bounded()))
                            .collect(),
                    );
                }
                // Usage and bounds belong on records, not entities.
                Some(role @ (Resource::Usage { .. } | Resource::Bounds { .. })) => {
                    self.errors.push(ResourceError::MisplacedRole {
                        location: cursor.to_string(),
                        role: role.kind(),
                        element: "an entity",
                    });
                }
                None => {}
            },
            Element::Record(record) => match self.role(cursor, record.annotations()) {
                Some(Resource::Usage { resource }) => {
                    self.usage_records.insert(
                        record.name().clone(),
                        UsageRecord {
                            resource,
                            claims: record.fields().map(|field| field.name().clone()).collect(),
                            location: cursor.to_string(),
                        },
                    );
                }
                Some(Resource::Bounds { resource }) => {
                    self.bounds_records.insert(
                        record.name().clone(),
                        BoundsRecord {
                            resource,
                            fields: record.fields().map(|field| field.name().clone()).collect(),
                            location: cursor.to_string(),
                        },
                    );
                }
                // A definition belongs on an entity, not a record.
                Some(role @ Resource::Definition(_)) => {
                    self.errors.push(ResourceError::MisplacedRole {
                        location: cursor.to_string(),
                        role: role.kind(),
                        element: "a record",
                    });
                }
                None => {}
            },
            // Collect every record reference; usage records are validated in
            // `finish` once all roles are known.
            Element::DataType(DataType::Record(record)) => {
                self.record_refs.push(RecordRef {
                    record: record.clone(),
                    on_entity_ref: matches!(
                        cursor.previous(),
                        Some(Element::DataType(DataType::EntityRef { .. }))
                    ),
                    entity: enclosing_entity(cursor).map(|entity| {
                        (
                            entity.name().clone(),
                            entity.annotations().has_constraint(FsmConstraint::NAME),
                        )
                    }),
                    location: cursor.to_string(),
                });
            }
            _ => {}
        }
    }

    fn finish(self) -> Self::Output {
        let ResourceConstraint {
            mut errors,
            resources,
            usage_records,
            bounds_records,
            record_refs,
        } = self;

        // Requirements 5 and 6: a usage names a declared resource and claims
        // only that resource's capacities.
        for UsageRecord {
            resource,
            claims,
            location,
        } in usage_records.values()
        {
            let Some(capacities) = resources.get(resource) else {
                errors.push(ResourceError::UnknownResource {
                    location: location.clone(),
                    resource: resource.clone(),
                });
                continue;
            };
            for claim in claims {
                if !capacities.contains_key(claim) {
                    errors.push(ResourceError::UndeclaredCapacity {
                        location: location.clone(),
                        resource: resource.clone(),
                        capacity: claim.clone(),
                    });
                }
            }
        }

        // Requirements 4, 7 and 8: usage and bounds records are referenced
        // correctly. A usage rides on an entity reference and only an FSM may
        // use it. A bounds record appears only on its own resource's events.
        let mut non_fsm_seen = FxHashSet::default();
        for RecordRef {
            record,
            on_entity_ref,
            entity,
            location,
        } in &record_refs
        {
            if let Some(usage) = usage_records.get(record) {
                // Requirement 7: a usage is carried by an entity reference.
                if !on_entity_ref {
                    errors.push(ResourceError::UsageNotOnReference {
                        location: location.clone(),
                    });
                    continue;
                }
                // Requirement 4: only an FSM entity may use a resource.
                match entity {
                    Some((entity, is_fsm)) => {
                        if !is_fsm && non_fsm_seen.insert((entity.clone(), usage.resource.clone()))
                        {
                            errors.push(ResourceError::NonFsmUser {
                                entity: entity.clone(),
                                resource: usage.resource.clone(),
                            });
                        }
                    }
                    // A usage must ride on an entity's reference; none encloses it here.
                    None => errors.push(ResourceError::MisplacedRole {
                        location: location.clone(),
                        role: "usage",
                        element: "a non-entity reference",
                    }),
                }
            } else if let Some(bounds) = bounds_records.get(record) {
                // Requirement 8: a bounds record belongs to its resource, so the
                // entity referencing it must be that resource.
                let on_resource = matches!(entity, Some((entity, _)) if entity == &bounds.resource);
                if !on_resource {
                    errors.push(ResourceError::ForeignBounds {
                        location: location.clone(),
                        resource: bounds.resource.clone(),
                    });
                }
            }
        }

        // Requirement 3: a bounds record covers exactly its resource's bounded
        // capacities.
        for BoundsRecord {
            resource,
            fields,
            location,
        } in bounds_records.values()
        {
            let Some(capacities) = resources.get(resource) else {
                errors.push(ResourceError::UnknownResource {
                    location: location.clone(),
                    resource: resource.clone(),
                });
                continue;
            };
            // A bound is declared only for a bounded capacity.
            for field in fields {
                if capacities.get(field) != Some(&true) {
                    errors.push(ResourceError::UnboundedCapacity {
                        location: location.clone(),
                        resource: resource.clone(),
                        capacity: field.clone(),
                    });
                }
            }
            // Every bounded capacity is covered.
            for (capacity, bounded) in capacities {
                if *bounded && !fields.contains(capacity) {
                    errors.push(ResourceError::UncoveredCapacity {
                        location: location.clone(),
                        resource: resource.clone(),
                        capacity: capacity.clone(),
                    });
                }
            }
        }

        // Requirement 3: a resource with a bounded capacity has a bounds record.
        let bounded_resources: FxHashSet<Identifier> = bounds_records
            .values()
            .map(|bounds| bounds.resource.clone())
            .collect();
        for (resource, capacities) in &resources {
            if capacities.values().any(|bounded| *bounded) && !bounded_resources.contains(resource)
            {
                errors.push(ResourceError::MissingBounds {
                    resource: resource.clone(),
                });
            }
        }

        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.into_iter().next().unwrap()),
            _ => Err(ResourceError::Multiple(errors)),
        }
    }
}

impl ResourceConstraint {
    /// The resource role on `annotations`, recording an [`ResourceError::InvalidData`]
    /// and returning `None` when the role is present but malformed.
    fn role(&mut self, cursor: &Cursor, annotations: &Annotations) -> Option<Resource> {
        match parse_role(annotations) {
            None => None,
            Some(Err(message)) => {
                self.errors.push(ResourceError::InvalidData {
                    location: cursor.to_string(),
                    message,
                });
                None
            }
            Some(Ok(resource)) => Some(resource),
        }
    }

    /// Check a definition in isolation (requirements 1 and 2).
    fn check_definition(&mut self, resource: &Identifier, capacities: &Capacities) {
        // Requirement 1: at least one capacity.
        if capacities.is_empty() {
            self.errors.push(ResourceError::NoCapacities {
                resource: resource.clone(),
            });
        }
        // Requirement 2: a capacity's name is its key, keeping names unique.
        for (key, capacity) in capacities {
            if capacity.name() != key {
                self.errors.push(ResourceError::MismatchedCapacityName {
                    resource: resource.clone(),
                    key: key.clone(),
                    name: capacity.name().clone(),
                });
            }
        }
    }
}

impl Constraint for ResourceConstraint {
    const NAME: &'static str = Resource::NAME;
}

/// Read a resource role from `annotations`. `None` when no resource constraint
/// is present, `Some(Err(_))` when its data is missing or malformed.
fn parse_role(annotations: &Annotations) -> Option<Result<Resource, String>> {
    let constraint = annotations.constraint(Resource::NAME)?;
    Some(match constraint.data() {
        None => Err("constraint data is missing".to_string()),
        Some(raw) => serde_json::from_str::<Resource>(raw)
            .map_err(|e| format!("failed to decode resource: {e}")),
    })
}

/// The nearest entity enclosing the cursor, if any.
fn enclosing_entity<'s>(cursor: &Cursor<'s>) -> Option<&'s Entity> {
    cursor
        .elements()
        .iter()
        .rev()
        .find_map(|element| match *element {
            Element::Entity(entity) => Some(entity),
            _ => None,
        })
}

#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("{location}: invalid resource data: {message}")]
    InvalidData { location: String, message: String },
    #[error("{location}: a {role} role is misplaced on {element}")]
    MisplacedRole {
        location: String,
        role: &'static str,
        element: &'static str,
    },
    #[error("resource \"{resource}\" declares no capacities")]
    NoCapacities { resource: Identifier },
    #[error("resource \"{resource}\" capacity keyed \"{key}\" is named \"{name}\"")]
    MismatchedCapacityName {
        resource: Identifier,
        key: Identifier,
        name: Identifier,
    },
    #[error("{location}: names undeclared resource \"{resource}\"")]
    UnknownResource {
        location: String,
        resource: Identifier,
    },
    #[error("{location}: claims undeclared capacity \"{capacity}\" of resource \"{resource}\"")]
    UndeclaredCapacity {
        location: String,
        resource: Identifier,
        capacity: Identifier,
    },
    #[error("{location}: a usage record is used outside an entity reference")]
    UsageNotOnReference { location: String },
    #[error("entity \"{entity}\" uses resource \"{resource}\" but is not an FSM")]
    NonFsmUser {
        entity: Identifier,
        resource: Identifier,
    },
    #[error("{location}: bounds of resource \"{resource}\" used outside that resource's events")]
    ForeignBounds {
        location: String,
        resource: Identifier,
    },
    #[error(
        "{location}: bounds declare \"{capacity}\", which resource \"{resource}\" does not bound"
    )]
    UnboundedCapacity {
        location: String,
        resource: Identifier,
        capacity: Identifier,
    },
    #[error("{location}: bounds of resource \"{resource}\" omit bounded capacity \"{capacity}\"")]
    UncoveredCapacity {
        location: String,
        resource: Identifier,
        capacity: Identifier,
    },
    #[error("resource \"{resource}\" has a bounded capacity but no bounds record")]
    MissingBounds { resource: Identifier },
    #[error("multiple resource violations:\n{}", bullet_list(.0))]
    Multiple(Vec<ResourceError>),
}
