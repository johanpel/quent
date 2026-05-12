// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    entity::{
        EntityDeclaration, EntityHandle, EntityRef, Event, IntoErased, ObserverError, RegularRef,
    },
    fsm::Transition,
    resource::Capacity,
};
use quent_time::timestamp;
use quent_v2_model_macros::{Entity, Fsm, Resource, ResourceGroup, RootResourceGroup};

use std::{marker::PhantomData, sync::atomic::AtomicU16};

use uuid::Uuid;

// Any entity can be a resource group, which means that at least one of its
// events needs to carry resource group attributes.

// This is a tag type to convey an EntityRef is meant as a resource group
// parent. EntityRefs with this tag should only be able to be created from
// references to entities that are resource groups.
pub struct RgParentRef;

// Trait to convey an entity satisfies the requirements of a resource group.
pub trait ResourceGroupDeclaration: EntityDeclaration {}

// Tag type to convey a reference can be made to any type of resource group.
pub struct AnyRg;
impl EntityDeclaration for AnyRg {}
impl ResourceGroupDeclaration for AnyRg {}

// Typed conversion from a regular reference to a reference representing a
// resource group parent-child relation.
impl<R> From<EntityRef<R, RegularRef>> for EntityRef<R, RgParentRef>
where
    R: ResourceGroupDeclaration,
{
    fn from(value: EntityRef<R, RegularRef>) -> Self {
        Self {
            _entity: PhantomData,
            _ref_kind: PhantomData,
            id: value.id,
        }
    }
}

// Type-erasing conversion from a regular resource group reference to a
// reference representing a resource group parent-child relation.
//
// This variant is useful when a resource group parent-child relation supports
// multiple types of parents.
impl<R> IntoErased<EntityRef<AnyRg, RgParentRef>> for EntityRef<R, RegularRef>
where
    R: ResourceGroupDeclaration,
{
    fn into_erased(self) -> EntityRef<AnyRg, RgParentRef> {
        EntityRef {
            _entity: PhantomData,
            _ref_kind: PhantomData,
            id: self.id,
        }
    }
}
