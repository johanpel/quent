use crate::ir::attributes::{EntityRefKind, EntityRefTarget};

/// Trait to obtain the IR of a Rust type.
pub trait ModelValueType {
    fn model_value_type() -> ValueType;
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
        /// The type of entity this reference can target.
        target: EntityRefTarget,
        /// The semantic relation of this reference.
        kind: EntityRefKind,
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
