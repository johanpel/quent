// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    IrError, event::Event, identifier::Identifier, qualifications::Qualification,
    validator::qualifications::QualificationCheck,
};

/// IR of an Entity
#[derive(Debug, PartialEq)]
pub struct Entity {
    /// The name of the entity.
    pub name: Identifier,
    /// The [`Event`]s types that this entity can emit.
    pub events: Vec<Event>,
    /// The [`Qualification`]s of the entity.
    pub qualifications: Vec<Qualification>,

    /// The Rust path of the entity.
    pub rust_path: String,
}

impl Entity {
    pub fn new(
        name: Identifier,
        events: Vec<Event>,
        qualifications: Vec<Qualification>,
        rust_path: impl Into<String>,
    ) -> Self {
        Self {
            name,
            events,
            qualifications,
            rust_path: rust_path.into(),
        }
    }

    pub fn qualification<T>(&self) -> Result<&T, IrError>
    where
        T: QualificationCheck,
        for<'a> &'a T: TryFrom<&'a Qualification>,
    {
        self.qualifications
            .iter()
            .find_map(|q| <&T>::try_from(q).ok())
            .ok_or(IrError::MissingQualification)
    }
}
