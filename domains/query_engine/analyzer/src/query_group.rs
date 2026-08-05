// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_analyzer::{AnalyzerResult, Entity, resource::ResourceGroup};
use quent_events::Event;
use quent_query_engine_model::query_group::QueryGroupEvent;
use quent_query_engine_ui as ui;
use uuid::Uuid;

use crate::{api, event_store::EntityTimeline};

/// A QueryGroup is an entity that groups [`super::query::Query`]s
#[derive(Debug)]
pub struct QueryGroup {
    timeline: EntityTimeline,
    instance_name: Option<String>,
    engine_id: Option<Uuid>,
}

impl QueryGroup {
    pub fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self {
            timeline: EntityTimeline::try_new(id)?,
            instance_name: None,
            engine_id: None,
        })
    }

    pub fn push(&mut self, event: Event<QueryGroupEvent>) {
        self.timeline.push(event.timestamp);
        let QueryGroupEvent::Declaration {
            instance_name,
            engine_id,
        } = event.data;
        self.instance_name = Some(instance_name);
        self.engine_id = Some(engine_id.target);
    }

    pub fn to_ui(&self) -> ui::QueryGroup {
        ui::QueryGroup {
            id: self.timeline.id(),
            instance_name: self.instance_name.clone(),
            engine_id: self.engine_id,
        }
    }
}

impl Entity for QueryGroup {
    fn id(&self) -> Uuid {
        self.timeline.id()
    }
    fn type_name(&self) -> &str {
        "query group"
    }
    fn instance_name(&self) -> &str {
        self.instance_name.as_deref().unwrap_or_default()
    }
}

impl ResourceGroup for QueryGroup {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.engine_id
    }
}

impl api::QueryGroup for QueryGroup {
    fn to_ui(&self) -> ui::QueryGroup {
        QueryGroup::to_ui(self)
    }
}
