use crate::{
    ir::qualifications::{Qualification, resource::Resource},
    validator::qualifications::QualificationCheck,
};

impl QualificationCheck for Resource {
    fn qualifies(
        _model: &crate::ir::Model,
        entity: &crate::ir::entity::Entity,
    ) -> Result<(), super::QualificationError> {
        // Constraint: an entity can't be both a resource and a resource group.
        let _ = entity
            .qualifications
            .iter()
            .find(|q| matches!(q, Qualification::ResourceGroup(_)))
            .is_some();
        todo!()
    }
}
