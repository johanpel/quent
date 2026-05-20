use std::marker::PhantomData;

use quent_v2_model_ir::{
    attributes::{EntityRefKind, EntityRefTarget},
    value_type::{ModelEntityRefKind, ModelEntityRefTarget},
};
use uuid::Uuid;

use crate::EntityDeclaration;

/// Trait allowing specific [`EntityRef`]s to be type-erased.
pub trait IntoErased<T> {
    fn into_erased(self) -> T;
}

/// A reference to another entity.
///
/// `EntityType` defines the entity type to which this reference refers. This
/// can also be type-erased by using `EntityType = AnyEntity`, such that at
/// run-time, any entity's handle can be provided.
///
/// `RoleType` defines the role type of the reference. By default, it is a
/// regular reference (`RegularRef`) which holds no particular meaning. But, for
/// example, it can be set to [`super::resource_group::RgParentRef`] to specify
/// it carries a parent relation of a child resource group entity in the
/// resource hierarchy. The latter is a requirement of the ResourceGroup
/// qualification. One event MUST carry this field for an entity to qualify as a
/// ResourceGroup.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
pub struct EntityRef<EntityType: EntityDeclaration, RoleType = PlainRef> {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub _entity: PhantomData<EntityType>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub _role: PhantomData<RoleType>,

    pub id: Uuid,
}

/// Type to mark an [`EntityRef`] can be to any type of entity, to be determined
/// at run-time.
pub struct AnyEntity;
// Special case:
impl EntityDeclaration for AnyEntity {}

/// Type to mark an [`EntityRef`] of being of no particular meaning.
pub struct PlainRef;

impl<E: EntityDeclaration, R> Clone for EntityRef<E, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E: EntityDeclaration, R> Copy for EntityRef<E, R> {}

impl ModelEntityRefTarget for AnyEntity {
    fn model_entity_ref_target() -> EntityRefTarget {
        EntityRefTarget::Any
    }
}

impl ModelEntityRefKind for PlainRef {
    fn model_entity_ref_kind() -> EntityRefKind {
        EntityRefKind::Plain
    }
}
