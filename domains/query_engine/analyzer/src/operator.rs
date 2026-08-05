// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_analyzer::{AnalyzerResult, Entity, resource::ResourceGroup};
use quent_dynamic_attributes::{DynamicAttribute, DynamicAttributes};
use quent_events::Event;
use quent_query_engine_model::operator::OperatorEvent;
use quent_query_engine_ui as ui;
use quent_time::{TimeUnixNanoSec, span::SpanUnixNanoSec};
use uuid::Uuid;

use crate::{api, event_store::EntityTimeline};

/// An Operator in a Plan DAG.
#[derive(Debug)]
pub struct Operator {
    timeline: EntityTimeline,
    plan_id: Option<Uuid>,
    parent_operator_ids: Vec<Uuid>,
    instance_name: Option<String>,
    operator_type_name: Option<String>,
    custom_attributes: DynamicAttributes,
    statistics: Option<DynamicAttributes>,
    /// Computed externally from task spans.
    pub active_span: Option<SpanUnixNanoSec>,
}

impl Operator {
    pub fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self {
            timeline: EntityTimeline::try_new(id)?,
            plan_id: None,
            parent_operator_ids: Vec::new(),
            instance_name: None,
            operator_type_name: None,
            custom_attributes: DynamicAttributes::default(),
            statistics: None,
            active_span: None,
        })
    }

    pub fn push(&mut self, event: Event<OperatorEvent>) {
        self.timeline.push(event.timestamp);
        match event.data {
            OperatorEvent::Declaration {
                plan_id,
                parent_operator_ids,
                instance_name,
                type_name,
                custom_attributes,
            } => {
                self.plan_id = Some(plan_id.target);
                self.parent_operator_ids = parent_operator_ids
                    .into_iter()
                    .map(|id| id.target)
                    .collect();
                self.instance_name = Some(instance_name);
                self.operator_type_name = Some(type_name);
                self.custom_attributes = custom_attributes;
            }
            OperatorEvent::Statistics { custom_attributes } => {
                self.statistics = Some(custom_attributes);
            }
        }
    }

    /// The ID of the plan this operator belongs to.
    pub fn plan_id(&self) -> Option<Uuid> {
        self.plan_id
    }

    /// The span of time between the first moment an operator started processing
    /// an input, and the latest moment at which an operator finished producing
    /// an output (excluding any potential back-pressure).
    pub fn active_span(&self) -> Option<SpanUnixNanoSec> {
        self.active_span
    }

    pub fn operator_type_name(&self) -> Option<&str> {
        self.operator_type_name.as_deref()
    }

    pub fn to_ui(&self, epoch: TimeUnixNanoSec) -> ui::Operator {
        let custom_attributes = self
            .custom_attributes
            .iter()
            .map(|DynamicAttribute { key, value }| (key.clone(), value.clone()))
            .collect();

        let statistics = self
            .statistics
            .as_ref()
            .map(|attributes| ui::OperatorStatistics {
                custom_statistics: attributes
                    .iter()
                    .map(|DynamicAttribute { key, value }| {
                        (
                            key.clone(),
                            ui::OperatorStatistic {
                                value: value.clone(),
                                quantity: None,
                            },
                        )
                    })
                    .collect(),
            });

        ui::Operator {
            id: self.timeline.id(),
            plan_id: self.plan_id(),
            parent_operator_ids: self.parent_operator_ids.clone(),
            instance_name: self.instance_name.clone(),
            operator_type_name: self.operator_type_name.clone(),
            custom_attributes,
            statistics,
            active_span: self
                .active_span()
                .and_then(|span| span.try_to_secs_relative(epoch).ok()),
        }
    }
}

impl Entity for Operator {
    fn id(&self) -> Uuid {
        self.timeline.id()
    }
    fn type_name(&self) -> &str {
        "operator"
    }
    fn instance_name(&self) -> &str {
        self.instance_name.as_deref().unwrap_or_default()
    }
}

impl ResourceGroup for Operator {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.plan_id()
    }
}

impl api::Operator for Operator {
    fn plan_id(&self) -> Option<Uuid> {
        Operator::plan_id(self)
    }

    fn active_span(&self) -> Option<SpanUnixNanoSec> {
        Operator::active_span(self)
    }

    fn operator_type_name(&self) -> Option<&str> {
        Operator::operator_type_name(self)
    }

    fn to_ui(&self, epoch: TimeUnixNanoSec) -> ui::Operator {
        Operator::to_ui(self, epoch)
    }
}
