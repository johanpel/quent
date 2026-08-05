// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_analyzer::{AnalyzerError, AnalyzerResult, Entity, Span, resource::ResourceGroup};
use quent_events::Event;
use quent_query_engine_model::engine::{EngineEvent, EngineImplementationAttributes};
use quent_query_engine_ui as ui;
use quent_time::{span::SpanUnixNanoSec, try_to_secs_relative};
use uuid::Uuid;

use crate::{api, event_store::EntityTimeline};

/// The analyzer's Engine entity.
#[derive(Debug)]
pub struct Engine {
    timeline: EntityTimeline,
    implementation: Option<EngineImplementationAttributes>,
    instance_name: Option<String>,
}

impl Engine {
    pub fn new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self {
            timeline: EntityTimeline::try_new(id)?,
            implementation: None,
            instance_name: None,
        })
    }

    pub fn push(&mut self, event: Event<EngineEvent>) {
        self.timeline.push(event.timestamp);
        if let EngineEvent::Init {
            implementation,
            instance_name,
        } = event.data
        {
            self.implementation = Some(implementation);
            self.instance_name = instance_name;
        }
    }

    pub fn to_ui(&self) -> AnalyzerResult<ui::Engine> {
        let start = self.timeline.earliest();
        let end = self.timeline.latest();

        let duration_s = if let (Some(s), Some(e)) = (start, end) {
            Some(try_to_secs_relative(e, s)?)
        } else {
            None
        };

        Ok(ui::Engine {
            id: self.timeline.id(),
            start_time_unix_ns: start,
            duration_s,
            instance_name: self.instance_name.clone(),
            implementation: self.implementation.as_ref().map(Into::into),
        })
    }
}

impl Entity for Engine {
    fn id(&self) -> Uuid {
        self.timeline.id()
    }
    fn type_name(&self) -> &str {
        "engine"
    }
    fn instance_name(&self) -> &str {
        self.instance_name.as_deref().unwrap_or_default()
    }
}

impl Span for Engine {
    fn span(&self) -> AnalyzerResult<SpanUnixNanoSec> {
        if let (Some(start), Some(end)) = (self.timeline.earliest(), self.timeline.latest()) {
            Ok(SpanUnixNanoSec::try_new(start, end)?)
        } else {
            Err(AnalyzerError::IncompleteEntity(
                "engine does not have an exit timestamp".to_string(),
            ))
        }
    }
}

impl ResourceGroup for Engine {
    fn parent_group_id(&self) -> Option<Uuid> {
        None
    }
}

impl api::Engine for Engine {
    fn to_ui(&self) -> AnalyzerResult<ui::Engine> {
        Engine::to_ui(self)
    }
}
