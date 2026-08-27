// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Data batch FSM analysis types.

use quent_analyzer::{
    AnalyzerResult, Entity,
    fsm::{
        Transition,
        events::{FsmEvents, FsmEventsBuilder},
    },
};
use quent_simulator_instrumentation::data_batch::DataBatchTransition as ModelDataBatchTransition;
use quent_time::{TimeUnixNanoSec, Timestamp, to_secs_relative};
use quent_ui::{FiniteStateMachine, FsmTransition, FsmUsage};
use uuid::Uuid;

/// The reconstructed data batch FSM.
pub type DataBatch = FsmEvents<ModelDataBatchTransition>;

/// Builder for data batch FSMs.
pub type DataBatchBuilder = FsmEventsBuilder<ModelDataBatchTransition>;

/// Application-specific methods on the data batch FSM.
pub trait DataBatchExt {
    fn operator_id(&self) -> Option<Uuid>;
    fn init_timestamp(&self) -> Option<TimeUnixNanoSec>;
    fn try_to_ui_fsm(&self, epoch: TimeUnixNanoSec) -> AnalyzerResult<FiniteStateMachine>;
}

impl DataBatchExt for DataBatch {
    fn operator_id(&self) -> Option<Uuid> {
        self.first_data().and_then(|transition| match transition {
            ModelDataBatchTransition::Initialized(data) => Some(data.operator_id),
            _ => None,
        })
    }

    fn init_timestamp(&self) -> Option<TimeUnixNanoSec> {
        self.transitions().first().map(Timestamp::timestamp)
    }

    fn try_to_ui_fsm(&self, epoch: TimeUnixNanoSec) -> AnalyzerResult<FiniteStateMachine> {
        let transitions = self
            .transitions()
            .iter()
            .map(|transition| FsmTransition {
                name: transition.name().to_owned(),
                usages: transition
                    .usages
                    .iter()
                    .map(|usage| FsmUsage {
                        resource: usage.resource_id,
                        capacities: usage
                            .capacities
                            .iter()
                            .map(|capacity| (capacity.name.to_string(), capacity.value))
                            .collect(),
                    })
                    .collect(),
                timestamp: to_secs_relative(transition.timestamp(), epoch),
                attributes: transition.attributes(),
                derived_attributes: vec![],
                related_entities: vec![],
            })
            .collect();

        Ok(FiniteStateMachine {
            id: self.id(),
            type_name: self.type_name().to_owned(),
            instance_name: self.instance_name().to_owned(),
            transitions,
        })
    }
}
