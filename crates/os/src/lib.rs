// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Constraint for relating Quent entities to operating-system processes and
//! threads.
//!
//! `quent-os` marks entity types as OS processes or threads and defines the
//! canonical [`PROCESS_ID_PATH`] and [`THREAD_ID_PATH`] records used to correlate
//! their Quent UUIDs with native OS IDs. Each record has one optional ID field
//! per supported platform; the thread record also references its process.
//!
//! The constraint validates record shape, record placement, and that thread
//! entities are unit resources. Event consumers must enforce that exactly one
//! platform ID is present and that reported IDs and process references are
//! correct for the captured runtime.

use quent_constraints::{Constraint, utils::bullet_list};
use quent_resource::Resource;
use quent_schema::{
    Annotations, DataType, Entity, Identifier, Path, Record,
    visitor::{Cursor, Element, Visitor},
};
use rustc_hash::FxHashMap as Map;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod builder;

pub use builder::{BuildError, OsBuilder, OsParts};

/// Canonical schema path of the process ID record.
pub const PROCESS_ID_PATH: &str = "quent::os::ProcessId";
/// Canonical schema path of the thread ID record.
pub const THREAD_ID_PATH: &str = "quent::os::ThreadId";

/// Return the canonical process ID record path.
pub fn process_id_path() -> Path {
    PROCESS_ID_PATH
        .parse()
        .expect("canonical process ID path is valid")
}

/// Return the canonical thread ID record path.
pub fn thread_id_path() -> Path {
    THREAD_ID_PATH
        .parse()
        .expect("canonical thread ID path is valid")
}

/// Payload of the `quent.os.v0.1.0` constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Os {
    /// Declares an entity as an operating-system process.
    Process,
    /// Declares a unit-resource entity as an operating-system thread.
    Thread,
}

impl Os {
    /// Constraint identifier.
    pub const NAME: &'static str = "quent.os.v0.1.0";

    /// Encode this annotation as a constraint payload.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn constraint_data(self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self)
    }

    /// Return the valid OS annotation attached to `annotations`, if any.
    pub fn from_annotations(annotations: &Annotations) -> Option<Self> {
        let data = annotations.constraint(Self::NAME)?.data()?;
        serde_json::from_str(data).ok()
    }

    fn name(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Thread => "thread",
        }
    }
}

#[derive(Clone)]
struct Definition {
    role: Os,
    entity: Entity,
    resource: Option<Result<Resource, String>>,
}

#[derive(Clone)]
struct RecordUse {
    record: Path,
    entity: Option<Path>,
    location: String,
}

#[derive(Clone)]
struct NativeIdRecord {
    role: Os,
    record: Record,
}

/// Validates process and thread annotations and their native-ID records.
///
/// ## Requirements
///
/// 1. An [`Os::Process`] or [`Os::Thread`] payload may annotate only an
///    [`Entity`].
/// 2. The canonical process ID record has optional `linux_id: I32`,
///    `macos_id: I32`, and `windows_id: U32` fields.
/// 3. Every use of the canonical process ID record occurs within an event
///    of an entity annotated with [`Os::Process`].
/// 4. The canonical thread ID record has optional `linux_id: I32`,
///    `macos_id: U64`, and `windows_id: U32` fields, plus a `process` field of
///    type [`DataType::EntityRef`].
/// 5. Every use of the canonical thread ID record occurs within an event of
///    an entity annotated with [`Os::Thread`].
/// 6. Every entity annotated with [`Os::Thread`] is also a `quent.resource`
///    definition with no capacities.
#[derive(Default)]
pub struct OsConstraint {
    errors: Vec<OsError>,
    definitions: Map<Path, Definition>,
    native_id_records: Map<Path, NativeIdRecord>,
    record_uses: Vec<RecordUse>,
}

impl Visitor for OsConstraint {
    type Output = Result<(), OsError>;

    fn visit(&mut self, cursor: &Cursor) {
        match cursor.current() {
            Element::Entity(entity) => self.visit_entity(cursor, entity),
            Element::Record(record) => self.visit_record(cursor, record),
            Element::Schema(schema) => {
                self.reject_annotation(cursor, schema.annotations(), "a schema")
            }
            Element::Event(event) => {
                self.reject_annotation(cursor, event.annotations(), "an event")
            }
            Element::Field(field) => self.reject_annotation(cursor, field.annotations(), "a field"),
            Element::Annotations(annotations)
                if matches!(cursor.previous(), Some(Element::DataType(_))) =>
            {
                self.reject_annotation(cursor, annotations, "an entity-reference data type")
            }
            Element::DataType(DataType::Record(record)) => self.record_uses.push(RecordUse {
                record: record.clone(),
                entity: enclosing_entity(cursor).map(|entity| entity.path().clone()),
                location: cursor.to_string(),
            }),
            _ => {}
        }
    }

    fn finish(mut self) -> Self::Output {
        let native_id_records: Vec<_> = self.native_id_records.values().cloned().collect();
        for native_id in &native_id_records {
            self.validate_record_field(
                &native_id.record,
                "linux_id",
                &DataType::Option(Box::new(DataType::I32)),
            );
            self.validate_record_field(
                &native_id.record,
                "macos_id",
                &DataType::Option(Box::new(match native_id.role {
                    Os::Process => DataType::I32,
                    Os::Thread => DataType::U64,
                })),
            );
            self.validate_record_field(
                &native_id.record,
                "windows_id",
                &DataType::Option(Box::new(DataType::U32)),
            );
            if native_id.role == Os::Thread {
                self.validate_entity_ref_field(&native_id.record, "process");
            }
        }

        let record_uses = self.record_uses.clone();
        for record_use in &record_uses {
            if let Some(native_id) = self.native_id_records.get(&record_use.record) {
                self.validate_record_use(record_use, native_id.role);
            }
        }

        let definitions: Vec<_> = self.definitions.values().cloned().collect();
        for definition in &definitions {
            if definition.role == Os::Thread {
                self.validate_thread_resource(definition);
            }
        }

        match self.errors.len() {
            0 => Ok(()),
            1 => Err(self.errors.into_iter().next().unwrap()),
            _ => Err(OsError::Multiple(self.errors)),
        }
    }
}

impl Constraint for OsConstraint {
    const NAME: &'static str = Os::NAME;
}

impl OsConstraint {
    fn visit_entity(&mut self, cursor: &Cursor, entity: &Entity) {
        let Some(role) = self.decode(cursor, entity.annotations()) else {
            return;
        };
        self.definitions.insert(
            entity.path().clone(),
            Definition {
                role,
                entity: entity.clone(),
                resource: parse_resource_annotation(entity.annotations()),
            },
        );
    }

    fn visit_record(&mut self, cursor: &Cursor, record: &Record) {
        self.reject_annotation(cursor, record.annotations(), "a record");
        if let Some(role) = native_id_contract(record.path()) {
            self.native_id_records.insert(
                record.path().clone(),
                NativeIdRecord {
                    role,
                    record: record.clone(),
                },
            );
        }
    }

    fn decode(&mut self, cursor: &Cursor, annotations: &Annotations) -> Option<Os> {
        let constraint = annotations.constraint(Os::NAME)?;
        let Some(raw) = constraint.data() else {
            self.errors.push(OsError::InvalidData {
                location: cursor.to_string(),
                message: "constraint data is missing".to_string(),
            });
            return None;
        };
        match serde_json::from_str(raw) {
            Ok(annotation) => Some(annotation),
            Err(error) => {
                self.errors.push(OsError::InvalidData {
                    location: cursor.to_string(),
                    message: format!("failed to decode OS annotation: {error}"),
                });
                None
            }
        }
    }

    fn reject_annotation(
        &mut self,
        cursor: &Cursor,
        annotations: &Annotations,
        element: &'static str,
    ) {
        let Some(annotation) = self.decode(cursor, annotations) else {
            return;
        };
        self.errors.push(OsError::MisplacedAnnotation {
            location: cursor.to_string(),
            annotation: annotation.name(),
            element,
        });
    }

    fn validate_record_use(&mut self, record_use: &RecordUse, expected: Os) {
        let Some(entity) = &record_use.entity else {
            self.errors.push(OsError::NativeIdRecordOutsideEntity {
                location: record_use.location.clone(),
                record: record_use.record.clone(),
                expected: expected.name(),
            });
            return;
        };
        match self.definitions.get(entity) {
            None => self.errors.push(OsError::NativeIdRecordOnUnmarkedEntity {
                location: record_use.location.clone(),
                record: record_use.record.clone(),
                entity: entity.clone(),
                expected: expected.name(),
            }),
            Some(definition) if definition.role != expected => {
                self.errors.push(OsError::WrongNativeIdRecordOwner {
                    location: record_use.location.clone(),
                    record: record_use.record.clone(),
                    entity: entity.clone(),
                    expected: expected.name(),
                    actual: definition.role.name(),
                });
            }
            Some(_) => {}
        }
    }

    fn validate_record_field(&mut self, record: &Record, field: &str, expected: &DataType) {
        let field = Identifier::try_new(field).expect("static identifier is valid");
        let Some(actual) = record.field(&field) else {
            self.errors.push(OsError::MissingRecordField {
                record: record.path().clone(),
                field,
            });
            return;
        };
        if actual.ty() != expected {
            self.errors.push(OsError::InvalidRecordFieldType {
                record: record.path().clone(),
                field,
                expected: Box::new(expected.clone()),
                actual: Box::new(actual.ty().clone()),
            });
        }
    }

    fn validate_entity_ref_field(&mut self, record: &Record, field: &str) {
        let field = Identifier::try_new(field).expect("static identifier is valid");
        let Some(actual) = record.field(&field) else {
            self.errors.push(OsError::MissingRecordField {
                record: record.path().clone(),
                field,
            });
            return;
        };
        if !matches!(actual.ty(), DataType::EntityRef { .. }) {
            self.errors.push(OsError::InvalidRecordFieldType {
                record: record.path().clone(),
                field,
                expected: Box::new(DataType::EntityRef {
                    data: None,
                    annotations: Annotations::default(),
                }),
                actual: Box::new(actual.ty().clone()),
            });
        }
    }

    fn validate_thread_resource(&mut self, definition: &Definition) {
        let message = match &definition.resource {
            None => Some("resource annotation is missing".to_string()),
            Some(Err(message)) => Some(message.clone()),
            Some(Ok(Resource::Definition(capacities))) if capacities.is_empty() => None,
            Some(Ok(Resource::Definition(_))) => {
                Some("resource definition declares capacities".to_string())
            }
            Some(Ok(Resource::Bounds { .. } | Resource::Usage { .. })) => {
                Some("resource annotation is not a definition".to_string())
            }
        };
        if let Some(message) = message {
            self.errors.push(OsError::ThreadNotUnitResource {
                entity: definition.entity.path().clone(),
                message,
            });
        }
    }
}

fn native_id_contract(path: &Path) -> Option<Os> {
    if path == &process_id_path() {
        Some(Os::Process)
    } else if path == &thread_id_path() {
        Some(Os::Thread)
    } else {
        None
    }
}

fn enclosing_entity<'s>(cursor: &Cursor<'s>) -> Option<&'s Entity> {
    cursor
        .elements()
        .iter()
        .rev()
        .find_map(|element| match *element {
            Element::Entity(entity) => Some(entity),
            _ => None,
        })
}

fn parse_resource_annotation(annotations: &Annotations) -> Option<Result<Resource, String>> {
    let constraint = annotations.constraint(Resource::NAME)?;
    Some(match constraint.data() {
        None => Err("resource constraint data is missing".to_string()),
        Some(raw) => {
            serde_json::from_str(raw).map_err(|error| format!("failed to decode resource: {error}"))
        }
    })
}

/// Error produced when an OS constraint requirement is violated.
#[derive(Debug, Error)]
pub enum OsError {
    #[error("{location}: invalid OS constraint data: {message}")]
    InvalidData { location: String, message: String },
    #[error("{location}: OS annotation `{annotation}` cannot annotate {element}")]
    MisplacedAnnotation {
        location: String,
        annotation: &'static str,
        element: &'static str,
    },
    #[error("{location}: `{record}` native-ID record must be used by an OS {expected} entity")]
    NativeIdRecordOutsideEntity {
        location: String,
        record: Path,
        expected: &'static str,
    },
    #[error(
        "{location}: `{record}` native-ID record is used by unmarked entity `{entity}`, expected an OS {expected}"
    )]
    NativeIdRecordOnUnmarkedEntity {
        location: String,
        record: Path,
        entity: Path,
        expected: &'static str,
    },
    #[error(
        "{location}: `{record}` native-ID record is used by OS {actual} entity `{entity}`, expected an OS {expected}"
    )]
    WrongNativeIdRecordOwner {
        location: String,
        record: Path,
        entity: Path,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("{record}: native-ID record is missing field `{field}`")]
    MissingRecordField { record: Path, field: Identifier },
    #[error("{record}.{field}: expected {expected:?}, found {actual:?}")]
    InvalidRecordFieldType {
        record: Path,
        field: Identifier,
        expected: Box<DataType>,
        actual: Box<DataType>,
    },
    #[error("{entity}: OS thread must be a unit resource: {message}")]
    ThreadNotUnitResource { entity: Path, message: String },
    #[error("multiple OS constraint violations:\n{}", bullet_list(.0))]
    Multiple(Vec<OsError>),
}
