// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Support for data types expressible in the Quent IR.

#[cfg(feature = "ir")]
use quent_v2_model_ir::data_type::DataType as IrDataType;

/// Trait for types expressible in the Quent IR.
pub trait DataType {
    #[cfg(feature = "ir")]
    fn ir() -> IrDataType;
}

// Convenience macro for trivial impls
macro_rules! impl_data_type {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl DataType for $ty {
                #[cfg(feature = "ir")]
                fn ir() -> IrDataType { IrDataType::$variant }
            }
        )*
    };
}

impl_data_type! {
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
    quent_attributes::CustomAttributes => DynamicRecord,
}

impl<T: DataType> DataType for Option<T> {
    #[cfg(feature = "ir")]
    fn ir() -> IrDataType {
        IrDataType::Option(Box::new(T::ir()))
    }
}

impl<T: DataType> DataType for Vec<T> {
    #[cfg(feature = "ir")]
    fn ir() -> IrDataType {
        IrDataType::List(Box::new(T::ir()))
    }
}
