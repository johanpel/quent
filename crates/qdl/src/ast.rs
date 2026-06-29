//! Parsed representation of a QDL source file.
//!
//! This is the syntactic AST. Names are kept as raw strings here; validity (the
//! [`quent_schema::Identifier`] grammar, reference resolution) is checked during
//! lowering.

/// A whole QDL file: a model name and its top-level items.
#[derive(Debug, Clone, PartialEq)]
pub struct Ast {
    pub model: String,
    pub items: Vec<Item>,
}

/// A top-level declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Record(RecordDef),
    Entity(EntityDef),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordDef {
    pub docs: Option<String>,
    pub name: String,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityDef {
    pub docs: Option<String>,
    pub name: String,
    pub events: Vec<EventDef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventDef {
    pub docs: Option<String>,
    pub cardinality: Cardinality,
    pub name: String,
    pub payload: Vec<FieldDef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    Once,
    Multi,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub docs: Option<String>,
    pub name: String,
    pub ty: Ty,
}

/// A field type, prior to resolution against scalars and declared records.
///
/// Kept structural: `App` is any `head<arg>` application; whether `head` is a
/// valid generic (`Option`/`Vec`) is decided during lowering.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    /// A scalar, `Dynamic`, or a record reference. Resolved during lowering.
    Named(String),
    /// A generic application `head<arg>`, e.g. `Option<Plan>`.
    App(String, Box<Ty>),
}
