use crate::{
    ir::qualifications::resource_group::ResourceGroup,
    validator::qualifications::QualificationCheck,
};

impl QualificationCheck for ResourceGroup {
    fn qualifies(
        model: &crate::ir::Model,
        entity: &crate::ir::entity::Entity,
    ) -> Result<(), super::QualificationError> {
        todo!()
    }
}
