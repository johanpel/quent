// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_analyzer::{AnalyzerError, AnalyzerResult};
use quent_time::TimeUnixNanoSec;
use uuid::Uuid;

#[derive(Debug)]
pub(crate) struct EntityTimeline {
    id: Uuid,
    earliest: Option<TimeUnixNanoSec>,
    latest: Option<TimeUnixNanoSec>,
}

impl EntityTimeline {
    pub(crate) fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        if id.is_nil() {
            return Err(AnalyzerError::Validation(
                "entity id cannot be nil".to_string(),
            ));
        }
        Ok(Self {
            id,
            earliest: None,
            latest: None,
        })
    }

    pub(crate) fn push(&mut self, timestamp: TimeUnixNanoSec) {
        self.earliest = Some(
            self.earliest
                .map_or(timestamp, |value| value.min(timestamp)),
        );
        self.latest = Some(self.latest.map_or(timestamp, |value| value.max(timestamp)));
    }

    pub(crate) fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn earliest(&self) -> Option<TimeUnixNanoSec> {
        self.earliest
    }

    pub(crate) fn latest(&self) -> Option<TimeUnixNanoSec> {
        self.latest
    }
}
