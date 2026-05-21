// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    attributes::{EntityRefKind, EntityRefTarget},
    identifier::Identifier,
};

/// Trait to obtain the IR of a Rust type.
pub trait ModelValueType {
    fn model_value_type() -> ValueType;
}

/// Trait to obtain the IR of an [`crate::entity::EntityRef`] target.
pub trait ModelEntityRefTarget {
    fn model_entity_ref_target() -> EntityRefTarget;
}

/// Trait to obtain the IR of an [`quent_v2_model::EntityRef`] role.
pub trait ModelEntityRefScope {
    fn model_entity_ref_scope() -> EntityRefScope;
}

/// IR of the types of entities targeted by an entity reference.
#[derive(Debug, PartialEq)]
pub enum EntityRefTarget {
    /// The entity reference targets one specific entity type.
    Specific(Identifier),
    /// The entity reference targets any entity.
    Any,
    /// The entity reference targets an entity with some qualification.
    AnyQualified(QualificationKind),
}

///
pub enum EntityRefScope {
    ///
    Root,
    Resource
}

/// IR of the role of an entity reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EntityRefRole {
    /// The entity reference has no specialized meaning, so it carries no data
    Unit,
    ///
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
        /// The scope of the reference.
        scope_type: EntityRefScope,
        /// The type of the data associated with the role of this reference
        role_type: EntityRefRole,
    },
    /// A reference to an attributes set.
    Attributes(Identifier),
    /// A set of attributes determined by the instrumentation client at run-time.
    CustomAttributes,
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
