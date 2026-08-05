// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Query FSM reconstruction from schema-generated events.

use quent_analyzer::{
    AnalyzerError, AnalyzerResult, Entity,
    fsm::{Fsm, FsmUsages, Transition},
    resource::{CapacityValue, ResourceGroup, Usage, Using},
};
use quent_events::Event;
use quent_query_engine_model::query::QueryEvent;
use quent_query_engine_ui as ui;
use quent_time::{
    TimeOrderedCollector, TimeUnixNanoSec, Timestamp, span::SpanUnixNanoSec, try_to_secs_relative,
};
use uuid::Uuid;

use crate::api;

#[derive(Debug)]
pub struct QueryTransition {
    timestamp: TimeUnixNanoSec,
    data: QueryEvent,
}

impl Timestamp for QueryTransition {
    fn timestamp(&self) -> TimeUnixNanoSec {
        self.timestamp
    }
}

impl Transition for QueryTransition {
    fn name(&self) -> &str {
        match self.data {
            QueryEvent::Init { .. } => "init",
            QueryEvent::Planning => "planning",
            QueryEvent::Executing => "executing",
            QueryEvent::Exit => "exit",
        }
    }
}

pub struct QueryBuilder {
    id: Uuid,
    transitions: TimeOrderedCollector<QueryTransition>,
}

impl QueryBuilder {
    pub fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        if id.is_nil() {
            return Err(AnalyzerError::Validation(
                "fsm id cannot be nil".to_string(),
            ));
        }
        Ok(Self {
            id,
            transitions: TimeOrderedCollector::default(),
        })
    }

    pub fn push(&mut self, event: Event<QueryEvent>) {
        self.transitions.push(QueryTransition {
            timestamp: event.timestamp,
            data: event.data,
        });
    }
}

/// A reconstructed Query FSM with resource group support.
#[derive(Debug)]
pub struct Query {
    id: Uuid,
    transitions: Vec<QueryTransition>,
}

impl Query {
    pub fn from_builder(builder: QueryBuilder) -> AnalyzerResult<Self> {
        Ok(Self {
            id: builder.id,
            transitions: builder.transitions.into_inner(),
        })
    }

    pub fn query_group_id(&self) -> Option<Uuid> {
        self.transitions.iter().find_map(|transition| {
            if let QueryEvent::Init { query_group_id, .. } = &transition.data {
                Some(query_group_id.target)
            } else {
                None
            }
        })
    }

    pub fn declared_instance_name(&self) -> Option<&str> {
        self.transitions.iter().find_map(|transition| {
            if let QueryEvent::Init { instance_name, .. } = &transition.data {
                Some(instance_name.as_str())
            } else {
                None
            }
        })
    }

    pub fn to_ui(&self) -> AnalyzerResult<ui::Query> {
        let epoch = self.transitions.first().map(Timestamp::timestamp);
        let relative = |event: fn(&QueryEvent) -> bool| -> AnalyzerResult<Option<_>> {
            let Some(epoch) = epoch else {
                return Ok(None);
            };
            Ok(self
                .transitions
                .iter()
                .find(|transition| event(&transition.data))
                .map(|transition| try_to_secs_relative(transition.timestamp(), epoch))
                .transpose()?)
        };

        Ok(ui::Query {
            id: self.id(),
            query_group_id: self.query_group_id().unwrap_or(Uuid::nil()),
            instance_name: self.declared_instance_name().map(str::to_owned),
            start_unix_ns: epoch,
            planning_s: relative(|event| matches!(event, QueryEvent::Planning))?,
            executing_s: relative(|event| matches!(event, QueryEvent::Executing))?,
            completed_s: relative(|event| matches!(event, QueryEvent::Exit))?,
        })
    }
}

impl Entity for Query {
    fn id(&self) -> Uuid {
        self.id
    }

    fn type_name(&self) -> &str {
        "query"
    }

    fn instance_name(&self) -> &str {
        self.declared_instance_name().unwrap_or_default()
    }
}

impl Fsm for Query {
    type TransitionType = QueryTransition;

    fn len(&self) -> usize {
        self.transitions.len().saturating_sub(1)
    }

    fn transition(&self, index: usize) -> Option<&Self::TransitionType> {
        self.transitions.get(index)
    }
}

struct NoUsage;

impl<'a> Usage<'a> for NoUsage {
    fn entity_id(&self) -> Uuid {
        unreachable!()
    }

    fn resource_id(&self) -> Uuid {
        unreachable!()
    }

    fn capacities(&self) -> impl Iterator<Item = &'a CapacityValue> {
        std::iter::empty()
    }

    fn span(&self) -> SpanUnixNanoSec {
        unreachable!()
    }
}

impl<'a> FsmUsages<'a> for Query {
    fn usages_with_state_names(&'a self) -> impl Iterator<Item = (&'a str, impl Usage<'a>)> {
        std::iter::empty::<(&'a str, NoUsage)>()
    }
}

impl Using for Query {
    fn usages<'a>(&'a self) -> impl Iterator<Item = impl Usage<'a>> {
        std::iter::empty::<NoUsage>()
    }
}

impl ResourceGroup for Query {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.query_group_id()
    }
}

impl api::Query for Query {
    fn query_group_id(&self) -> Option<Uuid> {
        Query::query_group_id(self)
    }

    fn to_ui(&self) -> AnalyzerResult<ui::Query> {
        Query::to_ui(self)
    }
}
