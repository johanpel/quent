// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_analyzer::{AnalyzerError, AnalyzerResult, Entity, Span, resource::ResourceGroup};
use quent_events::Event;
use quent_query_engine_model::worker::WorkerEvent;
use quent_query_engine_ui as ui;
use quent_time::{TimeUnixNanoSec, span::SpanUnixNanoSec};
use uuid::Uuid;

use crate::{api, event_store::EntityTimeline};

/// A [`Worker`] is an [`Entity`] that executes `Query` `Plan`s.
#[derive(Debug)]
pub struct Worker {
    timeline: EntityTimeline,
    parent_engine_id: Option<Uuid>,
    instance_name: Option<String>,
}

impl Worker {
    pub fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self {
            timeline: EntityTimeline::try_new(id)?,
            parent_engine_id: None,
            instance_name: None,
        })
    }

    pub fn push(&mut self, event: Event<WorkerEvent>) {
        self.timeline.push(event.timestamp);
        if let WorkerEvent::Init {
            parent_engine_id,
            instance_name,
        } = event.data
        {
            self.parent_engine_id = Some(parent_engine_id.target);
            self.instance_name = Some(instance_name);
        }
    }

    pub fn to_ui(&self, _epoch: TimeUnixNanoSec) -> ui::Worker {
        ui::Worker {
            id: self.timeline.id(),
            parent_engine_id: self.parent_engine_id,
            instance_name: self.instance_name.clone(),
            start_unix_ns: self.timeline.earliest(),
            end_unix_ns: self.timeline.latest(),
        }
    }
}

impl Entity for Worker {
    fn id(&self) -> Uuid {
        self.timeline.id()
    }
    fn type_name(&self) -> &str {
        "worker"
    }
    fn instance_name(&self) -> &str {
        self.instance_name.as_deref().unwrap_or_default()
    }
}

impl Span for Worker {
    fn span(&self) -> AnalyzerResult<SpanUnixNanoSec> {
        if let (Some(start), Some(end)) = (self.timeline.earliest(), self.timeline.latest()) {
            Ok(SpanUnixNanoSec::try_new(start, end)?)
        } else {
            Err(AnalyzerError::IncompleteEntity(
                "worker does not have an init or exit timestamp".to_string(),
            ))
        }
    }
}

impl ResourceGroup for Worker {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_engine_id
    }
}

impl api::Worker for Worker {
    fn to_ui(&self, epoch: TimeUnixNanoSec) -> ui::Worker {
        Worker::to_ui(self, epoch)
    }
}
