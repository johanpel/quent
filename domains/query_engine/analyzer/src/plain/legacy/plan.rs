// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_analyzer::entity::EntityEvents;
use quent_analyzer::{AnalyzerResult, Entity, resource::ResourceGroup};
use quent_events::Event;
use quent_query_engine_model::plan;
use quent_query_engine_ui as ui;
use uuid::Uuid;

use crate::PlanEntity;

/// An event-backed plan DAG.
#[derive(Debug)]
pub struct Plan(EntityEvents<plan::Plan>);

impl Plan {
    pub fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self(EntityEvents::new(id)?))
    }

    pub fn push(&mut self, event: Event<plan::PlanEvent>) {
        self.0.push(event);
    }
}

impl PlanEntity for Plan {
    fn parent_query_id(&self) -> Option<Uuid> {
        self.0
            .data()
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.parent.query_id.map(|query| query.uuid()))
    }

    fn parent_plan_id(&self) -> Option<Uuid> {
        self.0
            .data()
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.parent.plan_id.map(|plan| plan.uuid()))
    }

    fn worker_id(&self) -> Option<Uuid> {
        self.0
            .data()
            .declaration
            .as_ref()
            .and_then(|d| d.worker_id.map(|r| r.uuid()))
    }

    fn edges(&self) -> impl Iterator<Item = (Uuid, Uuid)> + '_ {
        self.0
            .data()
            .declaration
            .as_ref()
            .map(|declaration| declaration.edges.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|edge| (edge.source.uuid(), edge.target.uuid()))
    }

    fn to_ui(&self) -> ui::Plan {
        let parent = self.parent_query_id().or(self.parent_plan_id());

        ui::Plan {
            id: self.0.id(),
            instance_name: self
                .0
                .data()
                .declaration
                .as_ref()
                .map(|d| d.instance_name.clone()),
            parent,
            worker_id: self.worker_id(),
            edges: self
                .edges()
                .map(|(source, target)| ui::Edge { source, target })
                .collect(),
        }
    }
}

impl Entity for Plan {
    fn id(&self) -> Uuid {
        self.0.id()
    }

    fn type_name(&self) -> &str {
        "plan"
    }

    fn instance_name(&self) -> &str {
        self.0
            .data()
            .declaration
            .as_ref()
            .map(|d| d.instance_name.as_str())
            .unwrap_or_default()
    }
}

impl ResourceGroup for Plan {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.worker_id()
            .or(self.parent_query_id())
            .or(self.parent_plan_id())
    }
}
