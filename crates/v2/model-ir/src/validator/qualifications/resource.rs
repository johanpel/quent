// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    entity::Entity,
    qualifications::resource::Resource,
    validator::qualifications::{IrError, QualificationCheck},
};

impl QualificationCheck for Resource {
    fn qualifies(_entity: &Entity) -> Result<(), IrError> {
        Ok(())
    }
}
