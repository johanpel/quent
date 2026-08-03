// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_schema::{
    Annotations, DataType, Field, Identifier, Record,
    builder::{BuilderError, RecordBuilder},
};
use thiserror::Error;

use crate::{Os, process_id_path, thread_id_path};

/// The entity annotation and canonical native-ID record produced by an [`OsBuilder`].
pub struct OsParts {
    /// Constraint payload to place on the process or thread entity.
    pub definition: Os,
    /// Canonically named record that carries the entity's native OS ID.
    pub id: Record,
}

/// Builds an OS entity annotation and its canonical native-ID record.
pub struct OsBuilder {
    role: Os,
}

impl OsBuilder {
    /// Start a process definition using the canonical process ID record.
    pub fn process() -> Self {
        Self { role: Os::Process }
    }

    /// Start a thread definition using the canonical thread ID record.
    pub fn thread() -> Self {
        Self { role: Os::Thread }
    }

    /// Build the entity annotation and canonical native-ID record.
    ///
    /// # Errors
    ///
    /// Returns an error if the record cannot be built.
    pub fn build(self) -> Result<OsParts, BuildError> {
        let path = match self.role {
            Os::Process => process_id_path(),
            Os::Thread => thread_id_path(),
        };
        let mut record = RecordBuilder::new(path);
        for (name, ty) in [
            ("linux_id", DataType::I32),
            (
                "macos_id",
                match self.role {
                    Os::Process => DataType::I32,
                    Os::Thread => DataType::U64,
                },
            ),
            ("windows_id", DataType::U32),
        ] {
            record = record.with_field(Field::new(
                Identifier::try_new(name).expect("static identifier is valid"),
                DataType::Option(Box::new(ty)),
                Annotations::default(),
            ));
        }
        if self.role == Os::Thread {
            record = record.with_field(Field::new(
                Identifier::try_new("process").expect("static identifier is valid"),
                DataType::EntityRef {
                    data: None,
                    annotations: Annotations::default(),
                },
                Annotations::default(),
            ));
        }
        Ok(OsParts {
            definition: self.role,
            id: record.build()?,
        })
    }
}

/// Error produced while building OS schema parts.
#[derive(Debug, Error)]
pub enum BuildError {
    #[error(transparent)]
    Schema(#[from] BuilderError),
}
