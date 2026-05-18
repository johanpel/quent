use crate::{
    Span,
    qualifications::{QualificationKind, QualificationRefKind},
    value_type::ValueType,
};

/// Trait to obtain the IR of a type representing an attribute set.
pub trait ModelAttributes {
    fn model_attributes() -> Attributes;
}

/// Definition of a field in an attribute set.
#[derive(Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: ValueType,
}

/// IR of a set of attributes.
// TODO(johanpel): consider naming this Record or something else
#[derive(Debug, PartialEq)]
pub struct Attributes {
    /// The name of the attributes.
    pub name: String,
    /// The fields of the attributes.
    pub fields: Vec<Field>,

    /// The Rust path to the attributes.
    pub rust_path: String,

    /// Optional span for use within proc macros
    pub span: Span,
}

impl Attributes {
    pub fn new(
        name: impl Into<String>,
        fields: Vec<Field>,
        rust_path: impl Into<String>,
    ) -> Self {
        Self::with_span(name, fields, rust_path, Span::default())
    }

    pub fn with_span(
        name: impl Into<String>,
        fields: Vec<Field>,
        rust_path: impl Into<String>,
        span: Span,
    ) -> Self {
        Self {
            name: name.into(),
            fields,
            rust_path: rust_path.into(),
            span,
        }
    }
}

/// IR of the types of entities targeted by an entity reference.
#[derive(Debug, PartialEq)]
pub enum EntityRefTarget {
    /// The entity reference targets one specific entity type.
    Specific(String),
    /// The entity reference targets any entity.
    Any,
    /// The entity reference targets an entity with some qualification.
    AnyQualified(QualificationKind),
}

/// IR of the role of an entity reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EntityRefKind {
    /// The entity reference has no specialized meaning.
    Plain,
    /// The entity reference has specialized meaning required by the referring entities' qualification.
    Qualification(QualificationRefKind),
}
