// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Constraint for relating Quent entities to operating-system processes and threads.
//!
//! An entity represents an OS process or thread when one of its `Once` events
//! carries the canonical [`process_path`] or [`thread_path`] record. Each record
//! provides a normalized native ID; the thread record also references its
//! process.
//!
//! Event producers must enforce that reported IDs and process references are
//! correct for the captured runtime.

use quent_constraints::{
    Constraint,
    utils::{RecordValidationError, RecordValidator, bullet_list},
};
use quent_schema::{
    Cardinality, DataType, Identifier, Path, Record,
    visitor::{Cursor, Element, Visitor},
};
use rustc_hash::FxHashMap as Map;
use thiserror::Error;

mod record;

pub use record::{process_record, thread_record};

/// Validates the canonical process and thread records and their event usage.
///
/// The canonical record definitions are documented by [`process_record`] and
/// [`thread_record`].
///
/// ## Requirements
///
/// 1. A canonical process or thread record may be used only by an entity event.
/// 2. An entity may carry a canonical record in only one event, whose
///    cardinality must be [`Cardinality::Once`].
/// 3. An entity may not carry both canonical records.
#[derive(Default)]
pub struct OsConstraint {
    errors: Vec<OsError>,
    process_record: Option<Record>,
    thread_record: Option<Record>,
    os_record_uses: Vec<OsRecordUse>,
}

impl Constraint for OsConstraint {
    const NAME: &'static str = "quent.os.v0.1.0";
}

/// Return the canonical schema path of the process record.
pub fn process_path() -> Path {
    Path::try_new(["quent", "os", "Process"]).expect("canonical process path is valid")
}

/// Return the canonical schema path of the thread record.
pub fn thread_path() -> Path {
    Path::try_new(["quent", "os", "Thread"]).expect("canonical thread path is valid")
}

/// The operating-system role represented by a Quent entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OsEntityRole {
    /// An operating-system process.
    Process,
    /// An operating-system thread.
    Thread,
}

impl OsEntityRole {
    /// Return the canonical record path for this role.
    pub fn record_path(self) -> Path {
        match self {
            Self::Process => process_path(),
            Self::Thread => thread_path(),
        }
    }

    /// Return the canonical record for this role.
    pub fn record(self) -> Record {
        match self {
            Self::Process => process_record(),
            Self::Thread => thread_record(),
        }
    }
}

#[derive(Clone)]
struct OsRecordUse {
    role: OsEntityRole,
    record: Path,
    entity: Option<Path>,
    event: Option<Identifier>,
    cardinality: Option<Cardinality>,
    location: String,
}

struct OsRecordEvent {
    event: Identifier,
    cardinality: Cardinality,
    location: String,
}

impl Visitor for OsConstraint {
    type Output = Result<(), OsError>;

    fn visit(&mut self, cursor: &Cursor) {
        match cursor.current() {
            Element::Record(record) => {
                if let Some(role) = entity_role_for_record(record.path()) {
                    match role {
                        OsEntityRole::Process => self.process_record = Some(record.clone()),
                        OsEntityRole::Thread => self.thread_record = Some(record.clone()),
                    }
                }
            }
            Element::DataType(DataType::Record(record)) => {
                let Some(role) = entity_role_for_record(record) else {
                    return;
                };
                let event = cursor.enclosing_event();
                self.os_record_uses.push(OsRecordUse {
                    role,
                    record: record.clone(),
                    entity: cursor
                        .enclosing_entity()
                        .map(|entity| entity.path().clone()),
                    event: event.map(|event| event.name().clone()),
                    cardinality: event.map(|event| event.cardinality()),
                    location: cursor.to_string(),
                });
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Self::Output {
        let os_records = [
            (OsEntityRole::Process, self.process_record.take()),
            (OsEntityRole::Thread, self.thread_record.take()),
        ];
        for (role, record) in os_records
            .into_iter()
            .filter_map(|(role, record)| record.map(|record| (role, record)))
        {
            let expected = role.record();
            self.errors.extend(
                RecordValidator::new(&expected)
                    .validate(&record)
                    .into_iter()
                    .map(OsError::InvalidOsRecord),
            );
        }

        self.validate_os_record_uses();

        match self.errors.len() {
            0 => Ok(()),
            1 => Err(self.errors.into_iter().next().unwrap()),
            _ => Err(OsError::Multiple(self.errors)),
        }
    }
}

impl OsConstraint {
    fn validate_os_record_uses(&mut self) {
        let mut events_by_entity_and_role: Map<(Path, OsEntityRole), Vec<OsRecordEvent>> =
            Map::default();

        for record_use in std::mem::take(&mut self.os_record_uses) {
            let (Some(entity), Some(event), Some(cardinality)) =
                (record_use.entity, record_use.event, record_use.cardinality)
            else {
                self.errors.push(OsError::OsRecordOutsideEvent {
                    location: record_use.location,
                    record: record_use.record,
                });
                continue;
            };

            let events = events_by_entity_and_role
                .entry((entity, record_use.role))
                .or_default();
            if events.iter().all(|use_| use_.event != event) {
                events.push(OsRecordEvent {
                    event,
                    cardinality,
                    location: record_use.location,
                });
            }
        }

        let mut roles_by_entity: Map<Path, Vec<OsEntityRole>> = Map::default();
        for ((entity, role), events) in events_by_entity_and_role {
            roles_by_entity
                .entry(entity.clone())
                .or_default()
                .push(role);

            for event in &events {
                if event.cardinality != Cardinality::Once {
                    self.errors.push(OsError::OsRecordEventNotOnce {
                        location: event.location.clone(),
                        entity: entity.clone(),
                        record: role.record_path(),
                    });
                }
            }

            if events.len() > 1 {
                self.errors.push(OsError::OsRecordUsedByMultipleEvents {
                    entity,
                    record: role.record_path(),
                    events: events.into_iter().map(|use_| use_.event).collect(),
                });
            }
        }

        for (entity, roles) in roles_by_entity {
            if roles.len() > 1 {
                self.errors.push(OsError::ConflictingEntityRoles { entity });
            }
        }
    }
}

fn entity_role_for_record(path: &Path) -> Option<OsEntityRole> {
    if path == &process_path() {
        Some(OsEntityRole::Process)
    } else if path == &thread_path() {
        Some(OsEntityRole::Thread)
    } else {
        None
    }
}

/// Error produced when an OS constraint requirement is violated.
#[derive(Debug, Error)]
pub enum OsError {
    #[error("{location}: OS record `{record}` may only be used by an entity event")]
    OsRecordOutsideEvent { location: String, record: Path },
    #[error("{location}: entity `{entity}` must carry OS record `{record}` in a `Once` event")]
    OsRecordEventNotOnce {
        location: String,
        entity: Path,
        record: Path,
    },
    #[error("{entity}: OS record `{record}` may be carried by only one event, found {events:?}")]
    OsRecordUsedByMultipleEvents {
        entity: Path,
        record: Path,
        events: Vec<Identifier>,
    },
    #[error("{entity}: an entity cannot represent both an OS process and an OS thread")]
    ConflictingEntityRoles { entity: Path },
    #[error(transparent)]
    InvalidOsRecord(#[from] RecordValidationError),
    #[error("multiple OS constraint violations:\n{}", bullet_list(.0))]
    Multiple(Vec<OsError>),
}
