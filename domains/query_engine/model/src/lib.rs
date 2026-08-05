// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Query engine domain events.

pub mod engine;
pub mod operator;
pub mod plan;
pub mod port;
pub mod query;
pub mod query_group;
pub mod worker;

pub use engine::{Engine, EngineEvent, EngineImplementationAttributes};
pub use operator::{Operator, OperatorEvent};
pub use plan::{Edge, Plan, PlanEvent, PlanParent};
pub use port::{Port, PortEvent};
pub use quent_events::{EntityRef, Event};
pub use query::{Query, QueryEvent};
pub use query_group::{QueryGroup, QueryGroupEvent};
pub use worker::{Worker, WorkerEvent};

/// Events emitted by the query-engine domain.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum QueryEngineEvent {
    Engine(EngineEvent),
    Worker(WorkerEvent),
    QueryGroup(QueryGroupEvent),
    Query(QueryEvent),
    Plan(PlanEvent),
    Operator(OperatorEvent),
    Port(PortEvent),
}

macro_rules! event_conversion {
    ($event:ty, $variant:ident) => {
        impl From<$event> for QueryEngineEvent {
            fn from(event: $event) -> Self {
                Self::$variant(event)
            }
        }
    };
}

event_conversion!(EngineEvent, Engine);
event_conversion!(WorkerEvent, Worker);
event_conversion!(QueryGroupEvent, QueryGroup);
event_conversion!(QueryEvent, Query);
event_conversion!(PlanEvent, Plan);
event_conversion!(OperatorEvent, Operator);
event_conversion!(PortEvent, Port);

/// The query-engine event model.
pub struct QueryEngine;

impl quent_events::Model for QueryEngine {
    const NAME: &'static str = "QueryEngine";
}

impl quent_events::ModelEvents for QueryEngine {
    type UmbrellaEvent = QueryEngineEvent;
}
