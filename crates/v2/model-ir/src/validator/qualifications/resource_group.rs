use crate::{
    attributes::EntityRefKind,
    entity::Entity,
    event::FieldRole,
    qualifications::{
        Qualification, QualificationRefKind, resource_group::ResourceGroup,
        resource_group::RgRefKind,
    },
    validator::qualifications::{QualificationCheck, QualificationError},
    value_type::ValueType,
};

impl<'a> TryFrom<&'a Qualification> for &'a ResourceGroup {
    type Error = ();

    fn try_from(value: &'a Qualification) -> Result<Self, Self::Error> {
        match value {
            Qualification::ResourceGroup(rg) => Ok(rg),
            _ => Err(()),
        }
    }
}

impl QualificationCheck for ResourceGroup {
    fn qualifies(entity: &Entity) -> Result<(), QualificationError> {
        let rg: &ResourceGroup = entity
            .qualification()
            .ok_or(QualificationError::NotSpecified)?;

        let mut violations: Vec<String> = vec![];
        let mut violation = |reason: String| {
            violations.push(format!(
                "entity {} does not qualify as ResourceGroup. {}",
                entity.name, reason
            ))
        };

        // Constraint: an entity can't be both a resource group and a resource.
        // TODO(johanpel): might need to relax this as a part of https://github.com/rapidsai/quent/issues/186
        if entity
            .qualifications
            .iter()
            .any(|q| matches!(q, Qualification::Resource(_)))
        {
            violation("cannot also qualify as a resource".to_string());
        }

        // Constraint:
        // Non-root: the parent field must exist exactly once and hold a resource group parent role reference.
        // Root: the parent field may not exist'
        let mut num_parent_fields: usize = 0;
        for event in &entity.events {
            for field in event.payload.iter().filter(|f| f.role == FieldRole::Parent) {
                if !is_rg_parent_ref_type(&field.ty) {
                    violation(format!(
                        "`parent` field of event `{}` must be of type `EntityRef<_, RgParentRef>`",
                        event.name
                    ));
                } else {
                    num_parent_fields += 1;
                }
            }
        }
        match (rg.is_root, num_parent_fields) {
            (true, 0) => {}
            (true, _n /* > 0 */) => violation(format!(
                "root resource group cannot have `{}` field",
                FieldRole::PARENT
            )),
            (false, 1) => {}
            (false, n /* == 0 || _n > 1 */) => violation(format!(
                "non-root resource group events have a total of {n} parent fields, but must have exactly one event with one parent reference field"
            )),
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(QualificationError::Violations(violations))
        }
    }
}

fn is_rg_parent_ref_type(ty: &ValueType) -> bool {
    matches!(
        ty,
        ValueType::EntityRef {
            role_type: EntityRefKind::Qualification(QualificationRefKind::ResourceGroup(
                RgRefKind::Parent
            )),
            ..
        }
    )
}

// TODO(johanpel): test
