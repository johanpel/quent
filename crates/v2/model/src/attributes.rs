use crate::ir::attributes::ValueType;

/// Trait to obtain the IR of a set of attributes.
pub trait ModelAttributes {
    fn attributes_def() -> crate::ir::attributes::AttributesDef;
}

/// Trait to obtain the IR of a Rust type.
pub trait ModelValueType {
    fn value_type() -> ValueType;
}

macro_rules! impl_value_type {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl ModelValueType for $ty {
                fn value_type() -> ValueType { ValueType::$variant }
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
}

impl ModelValueType for uuid::Uuid {
    fn value_type() -> ValueType {
        ValueType::Uuid
    }
}

impl ModelValueType for quent_attributes::RuntimeAttributes {
    fn value_type() -> ValueType {
        ValueType::CustomAttributes
    }
}

impl<T: ModelValueType> ModelValueType for Option<T> {
    fn value_type() -> ValueType {
        ValueType::Option(Box::new(T::value_type()))
    }
}

impl<T: ModelValueType> ModelValueType for Vec<T> {
    fn value_type() -> ValueType {
        ValueType::List(Box::new(T::value_type()))
    }
}
