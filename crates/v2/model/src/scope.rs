// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    Entity,
    entity_ref::{EntityRef, EntityRefRole, EntityRefRoleTarget, Plain},
};

/// Trait to convey an [`EntityDeclaration`] is a valid target for entity
/// references with the [`Scope`] role.
pub trait Scope: Entity {}

pub struct AnyScope;
impl Entity for AnyScope {}
impl Scope for AnyScope {}

pub struct ScopeParentRefRole;
impl EntityRefRole for ScopeParentRefRole {} // the scope parent ref is a role
impl<S: Scope> EntityRefRoleTarget<S> for ScopeParentRefRole {} // anything that is a scope, can be a target of a scope parent ref

// Typed conversion from a plain reference to a scope entity with the plain role
// to a parent reference to a scope entity.
impl<S> From<EntityRef<Plain, S>> for EntityRef<ScopeParentRefRole, S>
where
    S: Scope,
{
    fn from(scope_plain_ref: EntityRef<Plain, S>) -> Self {
        Self::new(scope_plain_ref.id(), ScopeParentRefRole)
    }
}
