use crate::{
    ir::qualifications::resource_group::ResourceGroup, validator::qualifications::QualificationCheck,
};

impl QualificationCheck for ResourceGroup {
    fn qualifies(
        model: &crate::ir::ModelDef,
        entity: &crate::ir::entity::EntityDef,
    ) -> Result<(), super::QualificationError> {
        todo!()
    }
}
