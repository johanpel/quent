// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Typed attributes whose keys are defined at runtime.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;

/// A group of [`DynamicAttribute`]s.
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "ts", derive(TS))]
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicStruct(pub Vec<DynamicAttribute>);

/// A sequence of [`DynamicValue`]s.
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "ts", derive(TS), ts(untagged))]
#[derive(Clone, Debug, PartialEq)]
pub enum DynamicList {
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    I64(Vec<i64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
    String(Vec<String>),
    Struct(Vec<DynamicStruct>),
    List(Vec<DynamicList>),
}

/// A [`DynamicAttribute`] value.
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "ts", derive(TS), ts(untagged))]
#[derive(Clone, Debug, PartialEq)]
pub enum DynamicValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    String(String),
    Struct(DynamicStruct),
    List(DynamicList),
}

/// Marks a dynamic attribute as having no value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DynamicNull;

/// Converts a value for insertion into [`DynamicAttributes`].
pub trait IntoDynamicAttributeValue {
    /// Converts the input to a nullable dynamic value.
    fn into_dynamic_attribute_value(self) -> Option<DynamicValue>;
}

impl<T> IntoDynamicAttributeValue for T
where
    T: Into<DynamicValue>,
{
    fn into_dynamic_attribute_value(self) -> Option<DynamicValue> {
        Some(self.into())
    }
}

impl IntoDynamicAttributeValue for DynamicNull {
    fn into_dynamic_attribute_value(self) -> Option<DynamicValue> {
        None
    }
}

macro_rules! impl_from_dynamic_value {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl From<$ty> for DynamicValue {
                fn from(value: $ty) -> Self {
                    Self::$variant(value)
                }
            }
        )*
    };
}

impl_from_dynamic_value! {
    u8 => U8,
    u16 => U16,
    u32 => U32,
    u64 => U64,
    i8 => I8,
    i16 => I16,
    i32 => I32,
    i64 => I64,
    f32 => F32,
    f64 => F64,
    String => String,
    DynamicStruct => Struct,
    DynamicList => List,
}

impl From<&str> for DynamicValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<bool> for DynamicValue {
    fn from(value: bool) -> Self {
        Self::U8(u8::from(value))
    }
}

macro_rules! impl_from_dynamic_list {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl From<Vec<$ty>> for DynamicValue {
                fn from(value: Vec<$ty>) -> Self {
                    Self::List(DynamicList::$variant(value))
                }
            }
        )*
    };
}

impl_from_dynamic_list! {
    u8 => U8,
    u16 => U16,
    u32 => U32,
    u64 => U64,
    i8 => I8,
    i16 => I16,
    i32 => I32,
    i64 => I64,
    f32 => F32,
    f64 => F64,
    String => String,
    DynamicStruct => Struct,
    DynamicList => List,
}

impl From<Vec<bool>> for DynamicValue {
    fn from(value: Vec<bool>) -> Self {
        Self::List(DynamicList::U8(value.into_iter().map(u8::from).collect()))
    }
}

/// A key-value pair.
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "ts", derive(TS))]
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicAttribute {
    pub key: String,
    pub value: Option<DynamicValue>,
}

impl DynamicAttribute {
    /// Create a new attribute with the given key and no value.
    pub fn null(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: None,
        }
    }

    /// Create an attribute with a u8 value.
    pub fn u8(key: impl Into<String>, value: u8) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::U8(value)),
        }
    }

    /// Create an attribute with a u16 value.
    pub fn u16(key: impl Into<String>, value: u16) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::U16(value)),
        }
    }

    /// Create an attribute with a u32 value.
    pub fn u32(key: impl Into<String>, value: u32) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::U32(value)),
        }
    }

    /// Create an attribute with a u64 value.
    pub fn u64(key: impl Into<String>, value: u64) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::U64(value)),
        }
    }

    /// Create an attribute with an i8 value.
    pub fn i8(key: impl Into<String>, value: i8) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::I8(value)),
        }
    }

    /// Create an attribute with an i16 value.
    pub fn i16(key: impl Into<String>, value: i16) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::I16(value)),
        }
    }

    /// Create an attribute with an i32 value.
    pub fn i32(key: impl Into<String>, value: i32) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::I32(value)),
        }
    }

    /// Create an attribute with an i64 value.
    pub fn i64(key: impl Into<String>, value: i64) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::I64(value)),
        }
    }

    /// Create an attribute with an f32 value.
    pub fn f32(key: impl Into<String>, value: f32) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::F32(value)),
        }
    }

    /// Create an attribute with an f64 value.
    pub fn f64(key: impl Into<String>, value: f64) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::F64(value)),
        }
    }

    /// Create an attribute with a String value.
    pub fn string(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::String(value.into())),
        }
    }

    /// Create an attribute with a struct value.
    pub fn structure(key: impl Into<String>, value: DynamicStruct) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::Struct(value)),
        }
    }

    /// Create an attribute with a list value.
    pub fn list(key: impl Into<String>, value: DynamicList) -> Self {
        Self {
            key: key.into(),
            value: Some(DynamicValue::List(value)),
        }
    }
}

/// A collection of attributes whose keys are defined at runtime.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize), serde(transparent))]
pub struct DynamicAttributes(pub Vec<DynamicAttribute>);

impl DynamicAttributes {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn add(&mut self, key: impl Into<String>, value: impl IntoDynamicAttributeValue) {
        self.0.push(DynamicAttribute {
            key: key.into(),
            value: value.into_dynamic_attribute_value(),
        });
    }

    pub fn into_vec(self) -> Vec<DynamicAttribute> {
        self.0
    }
}

impl std::ops::Deref for DynamicAttributes {
    type Target = Vec<DynamicAttribute>;
    fn deref(&self) -> &Vec<DynamicAttribute> {
        &self.0
    }
}

impl From<Vec<DynamicAttribute>> for DynamicAttributes {
    fn from(v: Vec<DynamicAttribute>) -> Self {
        Self(v)
    }
}

impl From<DynamicAttributes> for Vec<DynamicAttribute> {
    fn from(v: DynamicAttributes) -> Self {
        v.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_lists_are_supported() {
        let mut attributes = DynamicAttributes::new();
        attributes.add(
            "matrix",
            DynamicList::List(vec![
                DynamicList::U64(vec![1, 2]),
                DynamicList::U64(vec![3, 4]),
            ]),
        );

        assert_eq!(
            attributes[0],
            DynamicAttribute::list(
                "matrix",
                DynamicList::List(vec![
                    DynamicList::U64(vec![1, 2]),
                    DynamicList::U64(vec![3, 4]),
                ]),
            ),
        );
    }

    #[test]
    fn generic_add_converts_supported_values() {
        let mut attributes = DynamicAttributes::new();
        attributes.add("boolean", true);
        attributes.add("integer", 42_u64);
        attributes.add("float", 1.5_f64);
        attributes.add("string", "value");
        attributes.add("list", vec![1_i32, 2]);
        attributes.add(
            "structure",
            DynamicStruct(vec![DynamicAttribute::string("field", "value")]),
        );
        attributes.add("missing", DynamicNull);

        assert_eq!(attributes[0].value, Some(DynamicValue::U8(1)));
        assert_eq!(attributes[1].value, Some(DynamicValue::U64(42)));
        assert_eq!(attributes[2].value, Some(DynamicValue::F64(1.5)));
        assert_eq!(
            attributes[3].value,
            Some(DynamicValue::String("value".to_string()))
        );
        assert_eq!(
            attributes[4].value,
            Some(DynamicValue::List(DynamicList::I32(vec![1, 2])))
        );
        assert_eq!(attributes[6].value, None);
    }
}
