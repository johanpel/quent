// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::entity::{EntityDeclaration, EntityRef, IntoErased, PlainRef};

use std::marker::PhantomData;

/// Trait to convey an [`EntityDeclaration`] satisfies the requirements of being
/// considered a resource group declaration.
///
/// The requierement is that the entity has an event with attributes in which
/// the resource group's parent is set.
pub trait ResourceGroupDeclaration: EntityDeclaration {}

/// Tag type to convey an [`EntityRef`] refers to a resource group's parent.
///
/// [`EntityRefs`] with this tag should only be able to be created from
/// references to entities that are resource groups.
pub struct RgParentRef;

/// Tag type to convey an [`EntityRef`] can be made to any type of resource
/// group.
pub struct AnyRg;
impl EntityDeclaration for AnyRg {}
impl ResourceGroupDeclaration for AnyRg {}

// Typed conversion from a regular reference to a reference representing a
// resource group parent-child relation.
impl<R> From<EntityRef<R, PlainRef>> for EntityRef<R, RgParentRef>
where
    R: ResourceGroupDeclaration,
{
    fn from(value: EntityRef<R, PlainRef>) -> Self {
        Self {
            _entity: PhantomData,
            _role: PhantomData,
            id: value.id,
        }
    }
}

// Type-erasing conversion from a regular resource group reference to a
// reference representing a resource group parent-child relation.
//
// This variant is useful when a resource group parent-child relation supports
// multiple types of parents.
impl<R> IntoErased<EntityRef<AnyRg, RgParentRef>> for EntityRef<R, PlainRef>
where
    R: ResourceGroupDeclaration,
{
    fn into_erased(self) -> EntityRef<AnyRg, RgParentRef> {
        EntityRef {
            _entity: PhantomData,
            _role: PhantomData,
            id: self.id,
        }
    }
}
