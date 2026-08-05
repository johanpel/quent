// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Task FSM reconstruction from schema-generated events.

use quent_analyzer::{
    AnalyzerError, AnalyzerResult, Entity,
    fsm::{
        Fsm, FsmStateTypeDecl, FsmTransitionDecl, FsmTypeDecl, FsmTypeDeclaration, FsmUsages,
        Transition,
    },
    resource::{CapacityValue, Usage, Using},
};
use quent_dynamic_attributes::DynamicAttribute;
use quent_events::Event;
use quent_simulator_instrumentation::TaskEvent;
use quent_time::{
    TimeOrderedCollector, TimeUnixNanoSec, Timestamp, span::SpanUnixNanoSec, to_secs_relative,
};
use quent_ui::{FiniteStateMachine, FsmTransition, FsmUsage};
use smallvec::SmallVec;
use uuid::Uuid;

#[derive(Debug)]
pub struct TaskTransition {
    timestamp: TimeUnixNanoSec,
    pub data: TaskEvent,
    pub usages: SmallVec<[TaskUsageData; 2]>,
}

impl Timestamp for TaskTransition {
    fn timestamp(&self) -> TimeUnixNanoSec {
        self.timestamp
    }
}

impl Transition for TaskTransition {
    fn name(&self) -> &str {
        match &self.data {
            TaskEvent::Queueing { .. } => "queueing",
            TaskEvent::Allocating { .. } => "allocating",
            TaskEvent::Loading { .. } => "loading",
            TaskEvent::Computing { .. } => "computing",
            TaskEvent::Spilling { .. } => "spilling",
            TaskEvent::Sending { .. } => "sending",
            TaskEvent::Exit => "exit",
        }
    }

    fn attributes(&self) -> Vec<DynamicAttribute> {
        match &self.data {
            TaskEvent::Computing { input_bytes, .. } => {
                vec![DynamicAttribute::u64("input_bytes", *input_bytes)]
            }
            _ => Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct TaskUsageData {
    pub resource_id: Uuid,
    pub capacities: SmallVec<[CapacityValue; 1]>,
}

impl TaskUsageData {
    fn unit(resource_id: Uuid) -> Self {
        Self {
            resource_id,
            capacities: SmallVec::new(),
        }
    }

    fn bytes(resource_id: Uuid, bytes: u64) -> Self {
        Self {
            resource_id,
            capacities: SmallVec::from([CapacityValue::new("capacity_bytes", bytes)]),
        }
    }
}

pub struct TaskBuilder {
    id: Uuid,
    transitions: TimeOrderedCollector<TaskTransition>,
}

impl TaskBuilder {
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

    pub fn push(&mut self, event: Event<TaskEvent>) {
        let usages = match &event.data {
            TaskEvent::Queueing { .. } | TaskEvent::Exit => SmallVec::new(),
            TaskEvent::Allocating { use_thread } => {
                SmallVec::from_vec(vec![TaskUsageData::unit(use_thread.target)])
            }
            TaskEvent::Loading {
                use_thread,
                use_fs_to_mem,
                use_memory,
            } => SmallVec::from_vec(vec![
                TaskUsageData::unit(use_thread.target),
                TaskUsageData::bytes(use_fs_to_mem.target, use_fs_to_mem.data.bytes),
                TaskUsageData::bytes(use_memory.target, use_memory.data.bytes),
            ]),
            TaskEvent::Computing {
                use_thread,
                use_memory,
                ..
            } => SmallVec::from_vec(vec![
                TaskUsageData::unit(use_thread.target),
                TaskUsageData::bytes(use_memory.target, use_memory.data.bytes),
            ]),
            TaskEvent::Spilling {
                use_thread,
                use_mem_to_fs,
            } => SmallVec::from_vec(vec![
                TaskUsageData::unit(use_thread.target),
                TaskUsageData::bytes(use_mem_to_fs.target, use_mem_to_fs.data.bytes),
            ]),
            TaskEvent::Sending {
                use_thread,
                use_link,
            } => SmallVec::from_vec(vec![
                TaskUsageData::unit(use_thread.target),
                TaskUsageData::bytes(use_link.target, use_link.data.bytes),
            ]),
        };
        self.transitions.push(TaskTransition {
            timestamp: event.timestamp,
            data: event.data,
            usages,
        });
    }

    pub fn try_build(self) -> AnalyzerResult<Task> {
        Ok(Task {
            id: self.id,
            transitions: self.transitions.into_inner(),
        })
    }
}

#[derive(Debug)]
pub struct Task {
    id: Uuid,
    transitions: Vec<TaskTransition>,
}

impl Task {
    pub fn transitions(&self) -> &[TaskTransition] {
        &self.transitions
    }
}

impl Entity for Task {
    fn id(&self) -> Uuid {
        self.id
    }

    fn type_name(&self) -> &str {
        "task"
    }

    fn instance_name(&self) -> &str {
        ""
    }
}

impl Fsm for Task {
    type TransitionType = TaskTransition;

    fn len(&self) -> usize {
        self.transitions.len().saturating_sub(1)
    }

    fn transition(&self, index: usize) -> Option<&Self::TransitionType> {
        self.transitions.get(index)
    }
}

struct TaskUsage<'a> {
    entity_id: Uuid,
    data: &'a TaskUsageData,
    span: SpanUnixNanoSec,
}

impl<'a> Usage<'a> for TaskUsage<'a> {
    fn entity_id(&self) -> Uuid {
        self.entity_id
    }

    fn resource_id(&self) -> Uuid {
        self.data.resource_id
    }

    fn capacities(&self) -> impl Iterator<Item = &'a CapacityValue> {
        self.data.capacities.iter()
    }

    fn span(&self) -> SpanUnixNanoSec {
        self.span
    }
}

impl<'a> FsmUsages<'a> for Task {
    fn usages_with_state_names(&'a self) -> impl Iterator<Item = (&'a str, impl Usage<'a>)> {
        self.transitions.windows(2).flat_map(move |window| {
            let start = window[0].timestamp();
            let end = window[1].timestamp();
            let span = SpanUnixNanoSec::try_new(start, end).unwrap();
            window[0].usages.iter().map(move |data| {
                (
                    window[0].name(),
                    TaskUsage {
                        entity_id: self.id,
                        data,
                        span,
                    },
                )
            })
        })
    }
}

impl Using for Task {
    fn usages<'a>(&'a self) -> impl Iterator<Item = impl Usage<'a>> {
        self.transitions.windows(2).flat_map(move |window| {
            let span =
                SpanUnixNanoSec::try_new(window[0].timestamp(), window[1].timestamp()).unwrap();
            window[0].usages.iter().map(move |data| TaskUsage {
                entity_id: self.id,
                data,
                span,
            })
        })
    }
}

impl FsmTypeDeclaration for Task {
    fn fsm_type_declaration() -> FsmTypeDecl {
        let state = |name: &str, usages: &[&str]| FsmStateTypeDecl {
            name: name.to_string(),
            usages: usages.iter().map(|name| (*name).to_string()).collect(),
        };
        FsmTypeDecl {
            name: "task".to_string(),
            states: vec![
                state("queueing", &[]),
                state("allocating", &["use_thread"]),
                state("loading", &["use_thread", "use_fs_to_mem", "use_memory"]),
                state("computing", &["use_thread", "use_memory"]),
                state("spilling", &["use_thread", "use_mem_to_fs"]),
                state("sending", &["use_thread", "use_link"]),
            ],
            transitions: vec![
                FsmTransitionDecl::Entry("queueing".to_string()),
                FsmTransitionDecl::Transition("queueing".to_string(), "allocating".to_string()),
                FsmTransitionDecl::Transition("allocating".to_string(), "computing".to_string()),
                FsmTransitionDecl::Transition("allocating".to_string(), "loading".to_string()),
                FsmTransitionDecl::Transition("loading".to_string(), "computing".to_string()),
                FsmTransitionDecl::Transition("computing".to_string(), "sending".to_string()),
                FsmTransitionDecl::Transition("computing".to_string(), "spilling".to_string()),
                FsmTransitionDecl::Transition("computing".to_string(), "exit".to_string()),
                FsmTransitionDecl::Transition("spilling".to_string(), "allocating".to_string()),
                FsmTransitionDecl::Transition("sending".to_string(), "queueing".to_string()),
                FsmTransitionDecl::Exit("exit".to_string()),
            ],
        }
    }
}

/// Application-specific methods on the Task FSM.
pub trait TaskExt {
    fn operator_id(&self) -> Option<Uuid>;
    fn active_span(&self) -> Option<SpanUnixNanoSec>;
    fn try_to_ui_fsm(&self, epoch: TimeUnixNanoSec) -> AnalyzerResult<FiniteStateMachine>;
}

impl TaskExt for Task {
    fn operator_id(&self) -> Option<Uuid> {
        self.transitions.first().and_then(|transition| {
            if let TaskEvent::Queueing { operator_id } = &transition.data {
                Some(operator_id.target)
            } else {
                None
            }
        })
    }

    fn active_span(&self) -> Option<SpanUnixNanoSec> {
        let start = self.transitions.get(1)?.timestamp();
        let end = self.transitions.last()?.timestamp();
        SpanUnixNanoSec::try_new(start, end).ok()
    }

    fn try_to_ui_fsm(&self, epoch: TimeUnixNanoSec) -> AnalyzerResult<FiniteStateMachine> {
        let transitions = self
            .transitions
            .iter()
            .enumerate()
            .map(|(index, transition)| {
                let mut derived_attributes = Vec::new();
                if let TaskEvent::Computing { input_bytes, .. } = &transition.data
                    && let Some(next) = self.transitions.get(index + 1)
                {
                    let span_secs = (next.timestamp() - transition.timestamp()) as f64 / 1e9;
                    if span_secs > 0.0 {
                        derived_attributes.push(DynamicAttribute::f64(
                            "bytes_per_sec",
                            *input_bytes as f64 / span_secs,
                        ));
                    }
                }
                Ok(FsmTransition {
                    name: transition.name().to_string(),
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
                    derived_attributes,
                })
            })
            .collect::<AnalyzerResult<Vec<_>>>()?;

        Ok(FiniteStateMachine {
            id: self.id(),
            type_name: self.type_name().to_string(),
            instance_name: self.instance_name().to_string(),
            transitions,
        })
    }
}
