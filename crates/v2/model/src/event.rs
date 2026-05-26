// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_v2_model_ir::event::{EntityRefTarget, EventFieldValueType};

use crate::{
    entity::Entity,
    entity_ref::{EntityRef, EntityRefRole, EntityRefRoleTarget},
    resource::{Resource, Usage},
};

/// Trait for types that can be used as event fields to carry built-in
/// semantics.
// TODO(johanpel): better name
pub trait EventField {
    #[cfg(feature = "ir")]
    fn ir() -> EventFieldValueType;
}

impl<R, E> EventField for EntityRef<R, E>
where
    E: Entity,
    R: EntityRefRole + EntityRefRoleTarget<E>,
{
    #[cfg(feature = "ir")]
    fn ir() -> EventFieldValueType {
        EventFieldValueType::EntityRef {
            role_type: R::ir(),
            entity_type: E::ir_ref_target(),
        }
    }
}

impl<R> EventField for Usage<R>
where
    R: Resource,
{
    #[cfg(feature = "ir")]
    fn ir() -> EventFieldValueType {
        let resource = match R::ir_ref_target() {
            EntityRefTarget::Specific(id) => id,
            EntityRefTarget::Any => {
                unreachable!("resource usages can only target resource entities")
            }
        };
        EventFieldValueType::ResourceUsage { resource }
    }
}

// Anything that implememts [`crate::attributes::ValueType`] is wrapped into
// [`EventFieldType::Payload`] if used as an event field type, as it is an
// application-specific type.
impl<T: crate::attributes::ValueType> EventField for T {
    #[cfg(feature = "ir")]
    fn ir() -> EventFieldValueType {
        EventFieldValueType::Payload(T::ir())
    }
}
