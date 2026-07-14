// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! The deserialized shape of a model file, format 1.
//!
//! These mirror the YAML one-to-one; `serde` fills them in and [`crate::lower`]
//! turns them into a schema. Names stay raw `String`s here (validated as
//! identifiers during lowering) and constraint/metadata payloads stay opaque
//! [`Value`]s (converted during lowering). Maps are [`IndexMap`]s so
//! declaration order is preserved.
//!
//! `doc`, `constraints`, and `metadata` are repeated on each element rather
//! than shared through one flattened struct: `serde`'s `deny_unknown_fields`,
//! which gives the unknown-key errors, does not work alongside `flatten`.

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

/// A constraint or metadata map, keyed by name with an opaque payload.
///
/// serde-saphyr deserializes each YAML payload straight into a JSON value.
pub(crate) type Anns = IndexMap<String, Value>;

/// A whole model file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Model {
    /// The format version. Only `1` is supported.
    pub(crate) quent: u32,
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) doc: Option<String>,
    #[serde(default)]
    pub(crate) constraints: Anns,
    #[serde(default)]
    pub(crate) metadata: Anns,
    #[serde(default)]
    pub(crate) records: IndexMap<String, Record>,
    #[serde(default)]
    pub(crate) entities: IndexMap<String, Entity>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    #[serde(default)]
    pub(crate) doc: Option<String>,
    #[serde(default)]
    pub(crate) constraints: Anns,
    #[serde(default)]
    pub(crate) metadata: Anns,
    #[serde(default)]
    pub(crate) fields: IndexMap<String, Field>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entity {
    #[serde(default)]
    pub(crate) doc: Option<String>,
    #[serde(default)]
    pub(crate) constraints: Anns,
    #[serde(default)]
    pub(crate) metadata: Anns,
    #[serde(default)]
    pub(crate) events: IndexMap<String, Event>,
}

/// An event, written either as the one-liner `name: once` or as a mapping with
/// a `once:`/`multi:` payload.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum Event {
    OneLiner(Cardinality),
    Body(Box<EventBody>),
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Cardinality {
    Once,
    Multi,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EventBody {
    #[serde(default)]
    pub(crate) doc: Option<String>,
    #[serde(default)]
    pub(crate) constraints: Anns,
    #[serde(default)]
    pub(crate) metadata: Anns,
    #[serde(default)]
    pub(crate) once: Option<Payload>,
    #[serde(default)]
    pub(crate) multi: Option<Payload>,
}

/// An event payload: field name to field, or null for no fields.
pub(crate) type Payload = Option<IndexMap<String, Field>>;

/// A field, written either as a bare type or as a mapping with a `type:` plus
/// annotations.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum Field {
    Bare(TypeExpr),
    Full(Box<FieldBody>),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FieldBody {
    pub(crate) r#type: TypeExpr,
    #[serde(default)]
    pub(crate) doc: Option<String>,
    #[serde(default)]
    pub(crate) constraints: Anns,
    #[serde(default)]
    pub(crate) metadata: Anns,
}

/// A field's type.
///
/// A bare name is a built-in ([`BuiltinType`]) or, failing that, a record.
/// `{ list: T }` and `{ option: T }` wrap another type, and the `{ ref:, data: }`
/// form is an entity reference. Composition nests through the YAML rather than a
/// string grammar.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum TypeExpr {
    Builtin(BuiltinType),
    Record(String),
    List(ListType),
    Option(OptionType),
    Ref(RefType),
}

/// The bare-name types with a fixed meaning. Variant names are the YAML
/// spellings (`u8`, `string`, `ref`, …) via `rename_all`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BuiltinType {
    Bool,
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
    String,
    Uuid,
    Dynamic,
    Ref,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListType {
    pub(crate) list: Box<TypeExpr>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OptionType {
    pub(crate) option: Box<TypeExpr>,
}

/// The entity reference form: `{ ref: , data: <type>, ... }`.
///
/// `ref` is required (it marks the form) and its value must be null; the value
/// is reserved for later syntax extensions.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RefType {
    pub(crate) r#ref: Value,
    #[serde(default)]
    pub(crate) data: Option<Box<TypeExpr>>,
    #[serde(default)]
    pub(crate) constraints: Anns,
    #[serde(default)]
    pub(crate) metadata: Anns,
}

impl From<Cardinality> for quent_schema::Cardinality {
    fn from(c: Cardinality) -> Self {
        match c {
            Cardinality::Once => quent_schema::Cardinality::Once,
            Cardinality::Multi => quent_schema::Cardinality::Multi,
        }
    }
}
