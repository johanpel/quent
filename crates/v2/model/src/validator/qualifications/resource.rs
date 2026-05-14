use crate::{
    ir::qualifications::resource::Resource, validator::qualifications::QualificationCheck,
};

impl QualificationCheck for Resource {
    fn qualifies(
        model: &crate::ir::Model,
        entity: &crate::ir::entity::Entity,
    ) -> Result<(), super::QualificationError> {
        todo!()
    }
}
