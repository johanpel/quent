// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Low-level PyO3 bindings over the `quent-schema` builders.
//!
//! These wrap the real builder + validation stack, so identifier and
//! duplicate-name errors surface as Python `ValueError`s and the schema is
//! validated through `quent-constraints`. The Pythonic declarative layer lives
//! in `quent/__init__.py`.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use quent_instrumentation_build::{Options, generate};
use quent_schema::builder::{
    BuilderError, EntityBuilder, EventBuilder, RecordBuilder, SchemaBuilder,
};
use quent_schema::{Annotations, Cardinality, DataType, Field, Identifier, Schema};

fn ident(name: &str) -> PyResult<Identifier> {
    Identifier::try_new(name)
        .map_err(|e| PyValueError::new_err(format!("invalid identifier {name:?}: {e}")))
}

fn builder_err(e: BuilderError) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// Parse the type mini-language: scalars, `option<T>`, `list<T>`, `dynamic`,
/// and bare names (resolved as record references during validation).
fn parse_type(expr: &str) -> PyResult<DataType> {
    let s = expr.trim();
    if let Some(inner) = wrapped(s, "option") {
        return Ok(DataType::Option(Box::new(parse_type(inner)?)));
    }
    if let Some(inner) = wrapped(s, "list") {
        return Ok(DataType::List(Box::new(parse_type(inner)?)));
    }
    Ok(match s {
        "bool" => DataType::Bool,
        "uuid" => DataType::Uuid,
        "string" => DataType::String,
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
        "dynamic" => DataType::DynamicRecord,
        "" => return Err(PyValueError::new_err("empty type expression")),
        other => DataType::Record(ident(other)?),
    })
}

fn wrapped<'s>(s: &'s str, head: &str) -> Option<&'s str> {
    s.strip_prefix(head)?
        .trim_start()
        .strip_prefix('<')?
        .strip_suffix('>')
}

/// Build payload/record fields from a kwargs dict of `name -> type expression`,
/// preserving insertion order.
fn fields_from_kwargs(kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Vec<Field>> {
    let mut fields = Vec::new();
    if let Some(dict) = kwargs {
        for (key, value) in dict.iter() {
            let name: String = key.extract()?;
            let ty: String = value.extract()?;
            fields.push(Field::new(
                ident(&name)?,
                parse_type(&ty)?,
                Annotations::default(),
            ));
        }
    }
    Ok(fields)
}

/// An entity under construction. Events are validated as they are added.
#[pyclass]
struct Entity {
    name: String,
    events: Vec<quent_schema::Event>,
}

#[pymethods]
impl Entity {
    #[pyo3(signature = (name, **payload))]
    fn once(&mut self, name: &str, payload: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        self.add_event(name, Cardinality::Once, payload)
    }

    #[pyo3(signature = (name, **payload))]
    fn multi(&mut self, name: &str, payload: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        self.add_event(name, Cardinality::Multi, payload)
    }
}

impl Entity {
    fn add_event(
        &mut self,
        name: &str,
        cardinality: Cardinality,
        payload: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let mut builder = EventBuilder::new(ident(name)?, cardinality);
        for field in fields_from_kwargs(payload)? {
            builder = builder.field(field).map_err(builder_err)?;
        }
        self.events.push(builder.build());
        Ok(())
    }
}

/// A model under construction.
#[pyclass]
struct Model {
    name: String,
    records: Vec<quent_schema::Record>,
    entities: Vec<Py<Entity>>,
}

#[pymethods]
impl Model {
    #[new]
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            records: Vec::new(),
            entities: Vec::new(),
        }
    }

    /// Declare a record. Fields are `name=type` kwargs.
    #[pyo3(signature = (name, **fields))]
    fn record(&mut self, name: &str, fields: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let mut builder = RecordBuilder::new(ident(name)?);
        for field in fields_from_kwargs(fields)? {
            builder = builder.field(field).map_err(builder_err)?;
        }
        self.records.push(builder.build());
        Ok(())
    }

    /// Declare an entity, returning a live handle to add events to.
    fn entity(&mut self, py: Python<'_>, name: &str) -> PyResult<Py<Entity>> {
        let entity = Py::new(
            py,
            Entity {
                name: name.to_string(),
                events: Vec::new(),
            },
        )?;
        self.entities.push(entity.clone_ref(py));
        Ok(entity)
    }

    /// Validate the model through the full constraint stack. Raises on failure.
    fn validate(&self, py: Python<'_>) -> PyResult<()> {
        self.build_schema(py).map(|_| ())
    }

    /// Generate the Rust instrumentation library into `out_dir`; returns the
    /// written path.
    fn generate_rust(&self, py: Python<'_>, out_dir: &str) -> PyResult<String> {
        let schema = self.build_schema(py)?;
        let opts = Options {
            event_derives: &["Debug"],
            record_derives: &["Debug"],
            out_dir: out_dir.into(),
            file_name: None,
        };
        let info = generate(&schema, &opts).map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(info.path.display().to_string())
    }
}

impl Model {
    fn build_schema(&self, py: Python<'_>) -> PyResult<Schema> {
        let mut schema = SchemaBuilder::new(ident(&self.name)?);
        schema = schema
            .records(self.records.iter().cloned())
            .map_err(builder_err)?;
        for entity in &self.entities {
            let entity = entity.borrow(py);
            let built = EntityBuilder::new(ident(&entity.name)?)
                .events(entity.events.iter().cloned())
                .map_err(builder_err)?
                .build();
            schema = schema.entity(built).map_err(builder_err)?;
        }
        let schema = schema.build();
        let report = quent_constraints::validate::<()>(&schema);
        if let Err(e) = report.base_constraints {
            return Err(PyValueError::new_err(e.to_string()));
        }
        Ok(schema)
    }
}

#[pymodule]
fn _quent(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Model>()?;
    m.add_class::<Entity>()?;
    Ok(())
}
