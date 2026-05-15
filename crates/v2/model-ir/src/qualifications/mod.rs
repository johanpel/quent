use crate::qualifications::{
    fsm::Fsm,
    resource::{Resource, ResourceRefKind},
    resource_group::{ResourceGroup, RgRefKind},
};

pub mod fsm;
pub mod resource;
pub mod resource_group;

/// IR of entity qualifications
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualificationKind {
    /// Finite-State-Machine
    ///
    /// The entity emits events in an order prescribed by a topology of states and transitions.
    Fsm,
    /// The entity qualifies  as a Resource.
    ///
    /// It is an FSM that goes through states determined by its capacities.
    Resource,
    /// The entity qualifies as a ResourceGroup.
    ///
    /// At least one event holds an attribute field that refers to its parent resource group.
    ResourceGroup,
}

/// IR of the types of entity references that have meaning specialized by the
/// qualification of the entity that emits them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualificationRefKind {
    Resource(ResourceRefKind),
    ResourceGroup(RgRefKind),
}

/// IR of a Qualification of an [`crate::ir::entity::Entity`].
///
/// Qualifications can be considered requirements of entity events. These
/// requirements can include constraints on any of the properties of events,
/// either their attributes or their order.
///
/// If these requirements are met, an entity is said to "qualify" as an "X".
///
/// Through these requirements, specializing semantics can be added to entities.
/// These specializations can be used to e.g. generate instrumentation API code
/// in a certain way. For example, by qualifying as a Finite-State-Machine, an
/// entity handle can be specialized to follow the Typestate pattern which
/// prevents illegal transitions at compile-time.
///
/// Qualifications can depend on each other. For example, in order for an entity
/// to qualify as a resource, it must also qualify as an FSM. The resource
/// qualification then puts additional constraints on the FSMs topology.
///
/// See [`QualificationKind`] for supported qualifications.
///
/// Qualifications are somewhat similar in spirit to Rust traits, but are named
/// differently to prevent the obvious terminology clashing.
///
/// Qualifications are mostly visible in the IR and code generation to capture
/// constraints.
#[derive(Debug, PartialEq, Eq)]
pub enum Qualification {
    Fsm(Fsm),
    Resource(Resource),
    ResourceGroup(ResourceGroup),
}

impl Qualification {
    pub fn kind(&self) -> QualificationKind {
        match self {
            Qualification::Fsm(_) => QualificationKind::Fsm,
            Qualification::Resource(_) => QualificationKind::Resource,
            Qualification::ResourceGroup(_) => QualificationKind::ResourceGroup,
        }
    }
}
