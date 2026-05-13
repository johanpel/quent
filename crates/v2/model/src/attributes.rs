/// Trait to obtain the IR representation of an attribute set.
pub trait HasAttributesDef {
    fn attributes_def() -> crate::ir::attributes::AttributesDef;
}

/// Trait to obtain the IR's value type of a Rust type.
pub trait HasValueType {
    fn value_type() -> crate::ir::attributes::ValueType;
}
