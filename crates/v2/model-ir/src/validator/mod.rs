// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

use crate::{
    entity::Entity,
    qualifications::{Qualification, fsm::Fsm, resource::Resource, resource_group::ResourceGroup},
    validator::qualifications::{QualificationCheck, QualificationError},
};

pub mod qualifications;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("qualification error: {0}")]
    Qualification(#[from] QualificationError),
}

impl Entity {
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let errs: Vec<_> = self
            .qualifications
            .iter()
            .filter_map(|q| {
                match q {
                    Qualification::Fsm(_) => Fsm::qualifies(self),
                    Qualification::Resource(_) => Resource::qualifies(self),
                    Qualification::ResourceGroup(_) => ResourceGroup::qualifies(self),
                }
                .err()
            })
            .map(ValidationError::from)
            .collect();
        if errs.is_empty() { Ok(()) } else { Err(errs) }
    }
}
