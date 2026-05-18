use thiserror::Error;

use crate::{
    Model,
    qualifications::{Qualification, fsm::Fsm, resource::Resource, resource_group::ResourceGroup},
    validator::qualifications::{QualificationCheck, QualificationError},
};

pub mod qualifications;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("qualification {0}")]
    Qualification(#[from] QualificationError),
}

impl Model {
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        // TODO: validate other stuff, like unknown identifiers etc.

        // Validate all entity qualifications
        let qualifications = self
            .entities
            .iter()
            .flat_map(|entity| entity.qualifications.iter().map(move |q| (entity, q)))
            .filter_map(|(entity, q)| {
                match q {
                    Qualification::Fsm(_) => Fsm::qualifies(entity),
                    Qualification::Resource(_) => Resource::qualifies(entity),
                    Qualification::ResourceGroup(_) => ResourceGroup::qualifies(entity),
                }
                .err()
            })
            .collect::<Vec<_>>();

        if qualifications.is_empty() {
            // and other checks resulted in no violations
            Ok(())
        } else {
            Err(qualifications
                .into_iter()
                .map(ValidationError::from)
                .collect())
        }
    }
}
