use crate::{
    Model,
    entity::Entity,
    qualifications::{Qualification, resource::Resource},
    validator::qualifications::{QualificationCheck, QualificationError},
};

impl QualificationCheck for Resource {
    fn qualifies(_model: &Model, entity: &Entity) -> Result<(), QualificationError> {
        // Constraint: an entity can't be both a resource and a resource group.
        if entity
            .qualifications
            .iter()
            .find(|q| matches!(q, Qualification::ResourceGroup(_)))
            .is_some()
        {
            Err(QualificationError::Violations(vec![format!(
                "entity {} cannot qualify as both resource and resource group",
                entity.name
            )]))
        } else {
            todo!()
        }
    }
}
