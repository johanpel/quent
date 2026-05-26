// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Support for attribute sets.
//!
//! Attribute sets are application-specific sets of attributes (key-value pairs)

#[cfg(feature = "ir")]
use quent_v2_model_ir::value_type::ValueType as IrValueType;

/// Trait for types expressible in the Quent IR.
pub trait ValueType {
    #[cfg(feature = "ir")]
    fn ir() -> IrValueType;
}

/// Trait for attribute sets expressible in the Quent IR.
pub trait Attributes {
    #[cfg(feature = "ir")]
    fn ir() -> quent_v2_model_ir::attributes::Attributes;
}

// TODO(johanpel): above can't be collapsed into one because a value type
// currently allows referring to an attribute set, wihch is basically a struct
// type def. use better words

// Convenience macro for trivial impls
macro_rules! impl_value_type {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl ValueType for $ty {
                #[cfg(feature = "ir")]
                fn ir() -> IrValueType { IrValueType::$variant }
            }
        )*
    };
}

impl_value_type! {
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
    uuid::Uuid => Uuid,
    quent_attributes::CustomAttributes => CustomAttributes,
}

impl<T: ValueType> ValueType for Option<T> {
    #[cfg(feature = "ir")]
    fn ir() -> IrValueType {
        IrValueType::Option(Box::new(T::ir()))
    }
}

impl<T: ValueType> ValueType for Vec<T> {
    #[cfg(feature = "ir")]
    fn ir() -> IrValueType {
        IrValueType::List(Box::new(T::ir()))
    }
}
