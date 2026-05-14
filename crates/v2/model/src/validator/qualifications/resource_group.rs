use crate::{
    ir::qualifications::resource_group::ResourceGroup,
    validator::qualifications::QualificationCheck,
};

impl QualificationCheck for ResourceGroup {
    fn qualifies(
        _model: &crate::ir::Model,
        _entity: &crate::ir::entity::Entity,
    ) -> Result<(), super::QualificationError> {
        todo!()
    }
}
