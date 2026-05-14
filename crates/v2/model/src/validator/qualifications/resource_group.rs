use crate::{
    ir::qualifications::{Qualification, resource_group::ResourceGroup},
    validator::qualifications::QualificationCheck,
};

impl QualificationCheck for ResourceGroup {
    fn qualifies(
        _model: &crate::ir::Model,
        entity: &crate::ir::entity::Entity,
    ) -> Result<(), super::QualificationError> {
        // Constraint: an entity can't be both a resource group and a resource.
        entity
            .qualifications
            .iter()
            .find(|q| matches!(q, Qualification::Resource(_)))
            .is_some();
        todo!()
    }
}
