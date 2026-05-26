// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    IrError,
    entity::Entity,
    qualifications::{Qualification, fsm::Fsm, resource::Resource},
    validator::qualifications::QualificationCheck,
};

pub mod qualifications;

impl Entity {
    pub fn validate(&self) -> Result<(), Vec<IrError>> {
        let errors = self
            .qualifications
            .iter()
            .filter_map(|q| {
                match q {
                    Qualification::Fsm(_) => Fsm::qualifies(self),
                    Qualification::Resource(_) => Resource::qualifies(self),
                }
                .err()
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
