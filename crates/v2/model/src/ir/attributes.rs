use crate::ir::qualifications::{QualificationKind, QualificationRefKind};

/// Definition of a field in an attribute set.
pub struct FieldDef {
    pub name: String,
    pub value_type: ValueType,
}

/// Definition of an attribute set.
pub struct AttributesDef {
    /// The name of the attributes.
    pub name: String,
    /// The fields of the attributes.
    pub fields: Vec<FieldDef>,
    /// The Rust path to the attributes.
    pub rust_path: String,
}

/// The types of entities targeted by an entity reference.
pub enum EntityRefTarget {
    /// The entity reference targets one specific entity type.
    Specific(String),
    /// The entity reference targets any entity.
    Any,
    /// The entity reference targets an entity with some qualification.
    AnyQualified(QualificationKind),
}

/// The semantics conveyed by an entity reference.
pub enum EntityRefKind {
    /// The entity reference has no specialized meaning.
    Plain,
    /// The entity reference has specialized meaning required by the referring entities' qualification.
    Qualification(QualificationRefKind),
}

/// Attribute value types.
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
