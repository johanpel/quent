use crate::{
    entity::Entity,
    qualifications::{Qualification, resource_group::ResourceGroup},
    validator::qualifications::{QualificationCheck, QualificationError},
};

impl QualificationCheck for ResourceGroup {
    fn qualifies(entity: &Entity) -> Result<(), QualificationError> {
        // Constraint: an entity can't be both a resource group and a resource.
        if entity
            .qualifications
            .iter()
            .any(|q| matches!(&q, Qualification::Resource(_)))
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
