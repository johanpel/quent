use thiserror::Error;

use crate::{Model, entity::Entity};

mod fsm;
mod resource;
mod resource_group;

#[derive(Debug, Error)]
pub enum QualificationError {
    #[error("entity doesn't hold the specified qualification.")]
    NotSpecified,
    #[error("entity fails to qualify: {}", .0.join("\n"))]
    Violations(Vec<String>),
}

/// A Qualifiation represents constraints of entity events.
pub trait QualificationCheck {
    /// Checks whether 'entity` qualifies as [`Self`].
    fn qualifies(entity: &Entity) -> Result<(), QualificationError>;
}
