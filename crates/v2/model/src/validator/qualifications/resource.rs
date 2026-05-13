use crate::{ir::qualifications::resource::Resource, validator::qualifications::Qualification};

impl Qualification for Resource {
    fn qualifies(
        model: &crate::ir::ModelDef,
        entity: &crate::ir::entity::EntityDef,
    ) -> Result<(), super::QualificationError> {
        todo!()
    }
}
