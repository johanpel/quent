// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    entity::EntityDeclaration,
    entity_ref::{EntityRef, IntoErased, PlainRef},
};

use quent_v2_model_ir::{
    attributes::{EntityRefKind, EntityRefTarget},
    qualifications::{QualificationKind, QualificationRefKind, resource_group::RgRefKind},
    value_type::{ModelEntityRefKind, ModelEntityRefTarget},
};
use std::marker::PhantomData;

/// Trait to convey an [`EntityDeclaration`] satisfies the requirements of being
/// considered a resource group declaration.
///
/// The requirement is that the entity has an event field in which the resource
/// group's parent is set, unless it is a root resource group.
pub trait ResourceGroupDeclaration: EntityDeclaration {
    /// Whether this resource group is a root, i.e. has no parent.
    const IS_ROOT: bool;
}

/// Marker to convey an [`EntityRef`] refers to a resource group's parent.
///
/// [`EntityRef`]s with this marker should only be able to be created from
/// references to entities that are resource groups.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RgParentRef;

/// Marker to convey an [`EntityRef`] can be made to any type of resource
/// group.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AnyRg;
impl EntityDeclaration for AnyRg {}
impl ResourceGroupDeclaration for AnyRg {
    const IS_ROOT: bool = false;
}

// Typed conversion from a regular reference to a reference representing a
// resource group parent-child relation.
impl<R> From<EntityRef<R, PlainRef>> for EntityRef<R, RgParentRef>
where
    R: ResourceGroupDeclaration,
{
    fn from(value: EntityRef<R, PlainRef>) -> Self {
        Self {
            _entity: PhantomData,
            role: RgParentRef,
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
            role: RgParentRef,
            id: self.id,
        }
    }
}

impl ModelEntityRefTarget for AnyRg {
    fn model_entity_ref_target() -> EntityRefTarget {
        EntityRefTarget::AnyQualified(QualificationKind::ResourceGroup)
    }
}

impl ModelEntityRefKind for RgParentRef {
    fn model_entity_ref_kind() -> EntityRefKind {
        EntityRefKind::Qualification(QualificationRefKind::ResourceGroup(RgRefKind::Parent))
    }
}
