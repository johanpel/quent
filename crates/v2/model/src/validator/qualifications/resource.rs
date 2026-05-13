use crate::{
    ir::qualifications::resource::Resource, validator::qualifications::QualificationCheck,
};

impl QualificationCheck for Resource {
    fn qualifies(
        model: &crate::ir::ModelDef,
        entity: &crate::ir::entity::EntityDef,
    ) -> Result<(), super::QualificationError> {
        todo!()
    }
}
