// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model_ir::event::{
    EntityRefTarget, EventFieldType, ModelEntityRefRole, ModelEntityRefTarget, ModelEventFieldType,
};

use crate::{
    entity::Entity,
    entity_ref::{EntityRef, EntityRefRole, EntityRefRoleTarget},
    resource::{Resource, Usage},
};

impl<R, E> ModelEventFieldType for EntityRef<R, E>
where
    E: Entity + ModelEntityRefTarget,
    R: EntityRefRole + EntityRefRoleTarget<E> + ModelEntityRefRole,
{
    fn model_event_field_type() -> EventFieldType {
        EventFieldType::EntityRef {
            role_type: R::model_entity_ref_role(),
            entity_type: E::model_entity_ref_target(),
        }
    }
}

impl<R> ModelEventFieldType for Usage<R>
where
    R: Resource + ModelEntityRefTarget,
{
    fn model_event_field_type() -> EventFieldType {
        let resource = match R::model_entity_ref_target() {
            EntityRefTarget::Specific(id) => id,
            EntityRefTarget::Any => {
                unreachable!("resource usages can only target resource entities")
            }
        };
        EventFieldType::ResourceUsage { resource }
    }
}
