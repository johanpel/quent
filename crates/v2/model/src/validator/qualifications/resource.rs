use crate::{
    ir::qualifications::resource::Resource, validator::qualifications::QualificationCheck,
};

impl QualificationCheck for Resource {
    fn qualifies(
        _model: &crate::ir::Model,
        _entity: &crate::ir::entity::Entity,
    ) -> Result<(), super::QualificationError> {
        todo!()
    }
}
