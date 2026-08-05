// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_analyzer::{AnalyzerResult, Entity, resource::ResourceGroup};
use quent_dynamic_attributes::{DynamicAttribute, DynamicAttributes};
use quent_events::Event;
use quent_query_engine_model::port::PortEvent;
use quent_query_engine_ui as ui;
use quent_time::TimeUnixNanoSec;
use uuid::Uuid;

use crate::{api, event_store::EntityTimeline};

/// A Port of an Operator in a Plan DAG.
#[derive(Debug)]
pub struct Port {
    timeline: EntityTimeline,
    operator_id: Option<Uuid>,
    instance_name: Option<String>,
    statistics: Option<DynamicAttributes>,
}

impl Port {
    pub fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self {
            timeline: EntityTimeline::try_new(id)?,
            operator_id: None,
            instance_name: None,
            statistics: None,
        })
    }

    pub fn push(&mut self, event: Event<PortEvent>) {
        self.timeline.push(event.timestamp);
        match event.data {
            PortEvent::Declaration {
                operator_id,
                instance_name,
            } => {
                self.operator_id = Some(operator_id.target);
                self.instance_name = Some(instance_name);
            }
            PortEvent::Statistics { custom_attributes } => {
                self.statistics = Some(custom_attributes);
            }
        }
    }

    /// The ID of the operator to which this port belongs.
    pub fn operator_id(&self) -> Option<Uuid> {
        self.operator_id
    }

    pub fn to_ui(&self, _epoch: TimeUnixNanoSec) -> ui::Port {
        ui::Port {
            id: self.timeline.id(),
            operator_id: self.operator_id(),
            instance_name: self.instance_name.clone(),
            statistics: self
                .statistics
                .as_ref()
                .map(|attributes| ui::PortStatistics {
                    custom_statistics: attributes
                        .iter()
                        .map(|DynamicAttribute { key, value }| (key.clone(), value.clone()))
                        .collect(),
                }),
        }
    }
}

impl Entity for Port {
    fn id(&self) -> Uuid {
        self.timeline.id()
    }
    fn type_name(&self) -> &str {
        "port"
    }
    fn instance_name(&self) -> &str {
        self.instance_name.as_deref().unwrap_or_default()
    }
}

impl ResourceGroup for Port {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.operator_id()
    }
}

impl api::Port for Port {
    fn operator_id(&self) -> Option<Uuid> {
        Port::operator_id(self)
    }

    fn to_ui(&self, epoch: TimeUnixNanoSec) -> ui::Port {
        Port::to_ui(self, epoch)
    }
}
