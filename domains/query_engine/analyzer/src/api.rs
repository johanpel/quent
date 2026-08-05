// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Domain interfaces for reconstructed query engine entities.

use quent_analyzer::{AnalyzerResult, Entity, Span, fsm::Fsm, resource::ResourceGroup};
use quent_query_engine_ui as ui;
use quent_time::{TimeUnixNanoSec, span::SpanUnixNanoSec};
use uuid::Uuid;

/// A reconstructed query engine.
pub trait Engine: Entity + ResourceGroup + Span {
    /// Convert this engine to its UI representation.
    fn to_ui(&self) -> AnalyzerResult<ui::Engine>;
}

/// A reconstructed query group.
pub trait QueryGroup: Entity + ResourceGroup {
    /// Convert this query group to its UI representation.
    fn to_ui(&self) -> ui::QueryGroup;
}

/// A reconstructed query engine worker.
pub trait Worker: Entity + ResourceGroup + Span {
    /// Convert this worker to its UI representation.
    fn to_ui(&self, epoch: TimeUnixNanoSec) -> ui::Worker;
}

/// A reconstructed query FSM.
pub trait Query: Entity + ResourceGroup + Fsm {
    /// Return the query group containing this query.
    fn query_group_id(&self) -> Option<Uuid>;

    /// Convert this query to its UI representation.
    fn to_ui(&self) -> AnalyzerResult<ui::Query>;
}

/// A directed edge between two ports in a plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanEdge {
    pub source: Uuid,
    pub target: Uuid,
}

/// A reconstructed query plan.
pub trait Plan: Entity + ResourceGroup {
    /// Return the query that owns this plan.
    fn query_id(&self) -> Option<Uuid>;

    /// Return the plan from which this plan was derived.
    fn parent_plan_id(&self) -> Option<Uuid>;

    /// Return the worker that executed this plan.
    fn worker_id(&self) -> Option<Uuid>;

    /// Return the directed edges between this plan's operator ports.
    fn edges(&self) -> impl ExactSizeIterator<Item = PlanEdge> + '_;

    /// Convert this plan to its UI representation.
    fn to_ui(&self) -> ui::Plan;
}

/// A reconstructed query plan operator.
pub trait Operator: Entity + ResourceGroup {
    /// Return the plan containing this operator.
    fn plan_id(&self) -> Option<Uuid>;

    /// Return the operator's active span.
    fn active_span(&self) -> Option<SpanUnixNanoSec>;

    /// Return the implementation-defined operator type name.
    fn operator_type_name(&self) -> Option<&str>;

    /// Convert this operator to its UI representation.
    fn to_ui(&self, epoch: TimeUnixNanoSec) -> ui::Operator;
}

/// A reconstructed query plan port.
pub trait Port: Entity + ResourceGroup {
    /// Return the operator containing this port.
    fn operator_id(&self) -> Option<Uuid>;

    /// Convert this port to its UI representation.
    fn to_ui(&self, epoch: TimeUnixNanoSec) -> ui::Port;
}
