use crate::{
    AnyEntity, AnyRg, EntityDeclaration, EntityRef, PlainRef, RgParentRef,
    ir::{
        attributes::{EntityRefKind, EntityRefTarget},
        qualifications::{QualificationKind, QualificationRefKind, resource_group::RgRefKind},
    },
};

/// Trait to obtain the IR of a Rust type.
pub trait ModelValueType {
    fn model_value_type() -> ValueType;
}

/// Trait to obtain the IR of an [`crate::entity::EntityRef`] target.
pub trait ModelEntityRefTarget {
    fn entity_ref_target() -> EntityRefTarget;
}

/// Trait to obtain the IR of an [`crate::entity::EntityRef`] role.
pub trait ModelEntityRefKind {
    fn entity_ref_kind() -> EntityRefKind;
}

/// Types of attribute values.
#[derive(Debug, PartialEq)]
pub enum ValueType {
    Bool,
    Uuid,
    String,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Option(Box<ValueType>),
    List(Box<ValueType>),
    /// A (run-time) reference to another entity.
    EntityRef {
        /// The entity type this reference can target.
        entity_type: EntityRefTarget,
        /// The role type used to bestow a certain meaning upon this reference.
        role_type: EntityRefKind,
    },
    /// A usage of a resource.
    Usage {
        resource: String,
    },
    /// A (compile-time) reference to an attributes set.
    Attributes(String),
    /// A set of attributes determined by the instrumentation client at run-time.
    CustomAttributes,
}

impl ValueType {
    pub fn attributes(ident: impl Into<String>) -> Self {
        Self::Attributes(ident.into())
    }
}

macro_rules! impl_model_value_type {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl ModelValueType for $ty {
                fn model_value_type() -> ValueType { ValueType::$variant }
            }
        )*
    };
}

impl_model_value_type! {
    bool   => Bool,
    String => String,
    u8     => U8,
    u16    => U16,
    u32    => U32,
    u64    => U64,
    i8     => I8,
    i16    => I16,
    i32    => I32,
    i64    => I64,
    f32    => F32,
    f64    => F64,
}

impl ModelValueType for uuid::Uuid {
    fn model_value_type() -> ValueType {
        ValueType::Uuid
    }
}

impl ModelValueType for quent_attributes::CustomAttributes {
    fn model_value_type() -> ValueType {
        ValueType::CustomAttributes
    }
}

impl<T: ModelValueType> ModelValueType for Option<T> {
    fn model_value_type() -> ValueType {
        ValueType::Option(Box::new(T::model_value_type()))
    }
}

impl<T: ModelValueType> ModelValueType for Vec<T> {
    fn model_value_type() -> ValueType {
        ValueType::List(Box::new(T::model_value_type()))
    }
}

impl<E, R> ModelValueType for EntityRef<E, R>
where
    E: EntityDeclaration + ModelEntityRefTarget,
    R: ModelEntityRefKind,
{
    fn model_value_type() -> ValueType {
        ValueType::EntityRef {
            entity_type: E::entity_ref_target(),
            role_type: R::entity_ref_kind(),
        }
    }
}

impl ModelEntityRefTarget for AnyEntity {
    fn entity_ref_target() -> EntityRefTarget {
        EntityRefTarget::Any
    }
}

impl ModelEntityRefTarget for AnyRg {
    fn entity_ref_target() -> EntityRefTarget {
        EntityRefTarget::AnyQualified(QualificationKind::ResourceGroup)
    }
}

impl ModelEntityRefKind for PlainRef {
    fn entity_ref_kind() -> EntityRefKind {
        EntityRefKind::Plain
    }
}

impl ModelEntityRefKind for RgParentRef {
    fn entity_ref_kind() -> EntityRefKind {
        EntityRefKind::Qualification(QualificationRefKind::ResourceGroup(RgRefKind::Parent))
    }
}
