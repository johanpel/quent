// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    entity::Entity,
    qualifications::{Qualification, resource::Resource},
    validator::qualifications::{QualificationCheck, QualificationError},
};

impl QualificationCheck for Resource {
    fn qualifies(entity: &Entity) -> Result<(), QualificationError> {
        // Constraint: an entity can't be both a resource and a resource group.
        if entity
            .qualifications
            .iter()
            .any(|q| matches!(&q, Qualification::ResourceGroup(_)))
        {
            Err(QualificationError::Violations(vec![format!(
                "entity {} cannot qualify as both resource and resource group",
                entity.name
            )]))
        } else {
            todo!()
        }
    }
}
