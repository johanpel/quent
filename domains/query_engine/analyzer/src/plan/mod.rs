// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_analyzer::{AnalyzerResult, Entity, resource::ResourceGroup};
use quent_events::Event;
use quent_query_engine_model::plan::{Edge, PlanEvent};
use quent_query_engine_ui as ui;
use uuid::Uuid;

use crate::{
    api::{self, PlanEdge},
    event_store::EntityTimeline,
};

pub mod tree;

/// A Directed-Acyclic-Graph of `Operator`s and [`Edge`]s.
///
/// Represents the dataflow starting at data sources, through operators
/// performing transformations, to an output.
#[derive(Debug)]
pub struct Plan {
    timeline: EntityTimeline,
    query_id: Option<Uuid>,
    parent_plan_id: Option<Uuid>,
    instance_name: Option<String>,
    edges: Vec<Edge>,
    worker_id: Option<Uuid>,
}

impl Plan {
    pub fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self {
            timeline: EntityTimeline::try_new(id)?,
            query_id: None,
            parent_plan_id: None,
            instance_name: None,
            edges: Vec::new(),
            worker_id: None,
        })
    }

    pub fn push(&mut self, event: Event<PlanEvent>) {
        self.timeline.push(event.timestamp);
        let PlanEvent::Declaration {
            parent,
            instance_name,
            edges,
            worker_id,
        } = event.data;
        self.query_id = Some(parent.query_id.target);
        self.parent_plan_id = parent.plan_id.map(|id| id.target);
        self.instance_name = Some(instance_name);
        self.edges = edges;
        self.worker_id = worker_id.map(|id| id.target);
    }

    /// The query that owns this plan.
    pub fn query_id(&self) -> Option<Uuid> {
        self.query_id
    }

    /// The plan from which this plan was derived.
    pub fn parent_plan_id(&self) -> Option<Uuid> {
        self.parent_plan_id
    }

    /// The worker that executed this plan, if any.
    pub fn worker_id(&self) -> Option<Uuid> {
        self.worker_id
    }

    /// The edges between operators of this plan.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn to_ui(&self) -> ui::Plan {
        ui::Plan {
            id: self.timeline.id(),
            instance_name: self.instance_name.clone(),
            parent: self.parent_plan_id.or(self.query_id),
            worker_id: self.worker_id(),
            edges: self
                .edges()
                .iter()
                .map(|e| ui::Edge {
                    source: e.source.target,
                    target: e.target.target,
                })
                .collect(),
        }
    }
}

impl Entity for Plan {
    fn id(&self) -> Uuid {
        self.timeline.id()
    }
    fn type_name(&self) -> &str {
        "plan"
    }
    fn instance_name(&self) -> &str {
        self.instance_name.as_deref().unwrap_or_default()
    }
}

impl ResourceGroup for Plan {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.query_id()
    }
}

impl api::Plan for Plan {
    fn query_id(&self) -> Option<Uuid> {
        Plan::query_id(self)
    }

    fn parent_plan_id(&self) -> Option<Uuid> {
        Plan::parent_plan_id(self)
    }

    fn worker_id(&self) -> Option<Uuid> {
        Plan::worker_id(self)
    }

    fn edges(&self) -> impl ExactSizeIterator<Item = PlanEdge> + '_ {
        Plan::edges(self).iter().map(|edge| PlanEdge {
            source: edge.source.target,
            target: edge.target.target,
        })
    }

    fn to_ui(&self) -> ui::Plan {
        Plan::to_ui(self)
    }
}
