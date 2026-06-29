//! Lowering from the QDL [`Ast`] to a [`quent_schema::Schema`].

use quent_schema::builder::{
    AnnotationsBuilder, EntityBuilder, EventBuilder, RecordBuilder, SchemaBuilder,
};
use quent_schema::{Annotations, Cardinality, DataType, Field, Identifier, Schema};

use crate::ast::{Ast, EntityDef, EventDef, FieldDef, Item, RecordDef, Ty};

/// Failure while lowering a parsed QDL file to a schema.
#[derive(Debug, thiserror::Error)]
pub enum LowerError {
    #[error("invalid identifier \"{name}\": {reason}")]
    Identifier { name: String, reason: String },
    #[error("duplicate name: {0}")]
    Duplicate(String),
    #[error("type arguments are only allowed on `Option` and `Vec`, found `{0}<...>`")]
    BadGeneric(String),
}

fn ident(name: &str) -> Result<Identifier, LowerError> {
    Identifier::try_new(name).map_err(|e| LowerError::Identifier {
        name: name.to_string(),
        reason: e.to_string(),
    })
}

fn annotations(docs: &Option<String>) -> Annotations {
    match docs {
        Some(d) => AnnotationsBuilder::new().docs(d.clone()).build(),
        None => Annotations::default(),
    }
}

/// Build a [`Schema`] from a parsed file. Does not run constraint validation.
pub fn lower(ast: &Ast) -> Result<Schema, LowerError> {
    let mut schema = SchemaBuilder::new(ident(&ast.model)?);
    for item in &ast.items {
        match item {
            Item::Record(r) => schema = schema.record(lower_record(r)?).map_err(dup)?,
            Item::Entity(e) => schema = schema.entity(lower_entity(e)?).map_err(dup)?,
        }
    }
    Ok(schema.build())
}

fn dup(e: quent_schema::builder::BuilderError) -> LowerError {
    LowerError::Duplicate(e.to_string())
}

fn lower_record(r: &RecordDef) -> Result<quent_schema::Record, LowerError> {
    let mut builder = RecordBuilder::new(ident(&r.name)?);
    for f in &r.fields {
        builder = builder.field(lower_field(f)?).map_err(dup)?;
    }
    Ok(builder.annotations(annotations(&r.docs)).build())
}

fn lower_entity(e: &EntityDef) -> Result<quent_schema::Entity, LowerError> {
    let mut builder = EntityBuilder::new(ident(&e.name)?);
    for ev in &e.events {
        builder = builder.event(lower_event(ev)?).map_err(dup)?;
    }
    Ok(builder.annotations(annotations(&e.docs)).build())
}

fn lower_event(ev: &EventDef) -> Result<quent_schema::Event, LowerError> {
    let cardinality = match ev.cardinality {
        crate::ast::Cardinality::Once => Cardinality::Once,
        crate::ast::Cardinality::Multi => Cardinality::Multi,
    };
    let mut builder = EventBuilder::new(ident(&ev.name)?, cardinality);
    for f in &ev.payload {
        builder = builder.field(lower_field(f)?).map_err(dup)?;
    }
    Ok(builder.annotations(annotations(&ev.docs)).build())
}

fn lower_field(f: &FieldDef) -> Result<Field, LowerError> {
    Ok(Field::new(
        ident(&f.name)?,
        lower_ty(&f.ty)?,
        annotations(&f.docs),
    ))
}

fn lower_ty(ty: &Ty) -> Result<DataType, LowerError> {
    Ok(match ty {
        Ty::App(head, inner) => match head.as_str() {
            "Option" => DataType::Option(Box::new(lower_ty(inner)?)),
            "Vec" => DataType::List(Box::new(lower_ty(inner)?)),
            other => return Err(LowerError::BadGeneric(other.to_string())),
        },
        Ty::Named(name) => match name.as_str() {
            "bool" => DataType::Bool,
            "Uuid" => DataType::Uuid,
            "String" => DataType::String,
            "u8" => DataType::U8,
            "u16" => DataType::U16,
            "u32" => DataType::U32,
            "u64" => DataType::U64,
            "i8" => DataType::I8,
            "i16" => DataType::I16,
            "i32" => DataType::I32,
            "i64" => DataType::I64,
            "f32" => DataType::F32,
            "f64" => DataType::F64,
            "Dynamic" => DataType::DynamicRecord,
            // Any other name is a reference to a declared record; existence is
            // checked by the base constraint validator.
            other => DataType::Record(ident(other)?),
        },
    })
}
