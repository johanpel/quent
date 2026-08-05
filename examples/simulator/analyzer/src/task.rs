// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Task FSM analysis types.

use quent_analyzer::{
    AnalyzerResult, Entity,
    fsm::{
        Transition,
        events::{FsmEvents, FsmEventsBuilder},
    },
};
use quent_dynamic_attributes::DynamicAttribute;
use quent_model::{
    FsmDef, ModelBuilder, StateDef, TransitionDef, TransitionEndpoint, UsageDef,
    analyze::{ExtractedCapacity, ExtractedUsage, TransitionInfo},
};
use quent_simulator_instrumentation as instr;
use quent_time::{TimeUnixNanoSec, Timestamp, span::SpanUnixNanoSec, to_secs_relative};
use quent_ui::{FiniteStateMachine, FsmTransition, FsmUsage};
use uuid::Uuid;

/// Analysis representation of a schema-generated task transition.
pub enum TaskTransition {
    Queueing {
        operator_id: instr::EntityRef<instr::Operator>,
    },
    Allocating {
        use_thread: instr::EntityRef<instr::Processor, instr::ProcessorUsage>,
    },
    Loading {
        use_thread: instr::EntityRef<instr::Processor, instr::ProcessorUsage>,
        use_fs_to_mem: instr::EntityRef<instr::StorageChannel, instr::StorageChannelUsage>,
        use_memory: instr::EntityRef<instr::Memory, instr::MemoryUsage>,
    },
    Computing {
        input_bytes: u64,
        use_thread: instr::EntityRef<instr::Processor, instr::ProcessorUsage>,
        use_memory: instr::EntityRef<instr::Memory, instr::MemoryUsage>,
    },
    Spilling {
        use_thread: instr::EntityRef<instr::Processor, instr::ProcessorUsage>,
        use_mem_to_fs: instr::EntityRef<instr::StorageChannel, instr::StorageChannelUsage>,
    },
    Sending {
        use_thread: instr::EntityRef<instr::Processor, instr::ProcessorUsage>,
        use_link: instr::EntityRef<instr::NetworkChannel, instr::NetworkChannelUsage>,
    },
    Exit,
}

impl From<instr::TaskEvent> for TaskTransition {
    fn from(event: instr::TaskEvent) -> Self {
        match event {
            instr::TaskEvent::Queueing { operator_id } => Self::Queueing { operator_id },
            instr::TaskEvent::Allocating { use_thread } => Self::Allocating { use_thread },
            instr::TaskEvent::Loading {
                use_thread,
                use_fs_to_mem,
                use_memory,
            } => Self::Loading {
                use_thread,
                use_fs_to_mem,
                use_memory,
            },
            instr::TaskEvent::Computing {
                input_bytes,
                use_thread,
                use_memory,
            } => Self::Computing {
                input_bytes,
                use_thread,
                use_memory,
            },
            instr::TaskEvent::Spilling {
                use_thread,
                use_mem_to_fs,
            } => Self::Spilling {
                use_thread,
                use_mem_to_fs,
            },
            instr::TaskEvent::Sending {
                use_thread,
                use_link,
            } => Self::Sending {
                use_thread,
                use_link,
            },
            instr::TaskEvent::Exit => Self::Exit,
        }
    }
}

fn unit_usage<E, T>(reference: &instr::EntityRef<E, T>) -> ExtractedUsage {
    ExtractedUsage {
        resource_id: reference.target,
        capacities: vec![ExtractedCapacity::unit()],
    }
}

fn capacity_usage<E>(reference: &instr::EntityRef<E, impl CapacityValue>) -> ExtractedUsage {
    ExtractedUsage {
        resource_id: reference.target,
        capacities: vec![ExtractedCapacity::new("bytes", reference.data.value())],
    }
}

trait CapacityValue {
    fn value(&self) -> u64;
}

impl CapacityValue for instr::MemoryUsage {
    fn value(&self) -> u64 {
        self.bytes
    }
}

impl CapacityValue for instr::StorageChannelUsage {
    fn value(&self) -> u64 {
        self.bytes
    }
}

impl CapacityValue for instr::NetworkChannelUsage {
    fn value(&self) -> u64 {
        self.bytes
    }
}

impl TransitionInfo for TaskTransition {
    fn state_name(&self) -> &'static str {
        match self {
            Self::Queueing { .. } => "queueing",
            Self::Allocating { .. } => "allocating",
            Self::Loading { .. } => "loading",
            Self::Computing { .. } => "computing",
            Self::Spilling { .. } => "spilling",
            Self::Sending { .. } => "sending",
            Self::Exit => "exit",
        }
    }

    fn usages(&self) -> Vec<ExtractedUsage> {
        match self {
            Self::Queueing { .. } | Self::Exit => vec![],
            Self::Allocating { use_thread } => vec![unit_usage(use_thread)],
            Self::Loading {
                use_thread,
                use_fs_to_mem,
                use_memory,
            } => vec![
                unit_usage(use_thread),
                capacity_usage(use_fs_to_mem),
                capacity_usage(use_memory),
            ],
            Self::Computing {
                use_thread,
                use_memory,
                ..
            } => vec![unit_usage(use_thread), capacity_usage(use_memory)],
            Self::Spilling {
                use_thread,
                use_mem_to_fs,
            } => vec![unit_usage(use_thread), capacity_usage(use_mem_to_fs)],
            Self::Sending {
                use_thread,
                use_link,
            } => vec![unit_usage(use_thread), capacity_usage(use_link)],
        }
    }

    fn instance_name(&self) -> Option<&str> {
        None
    }

    fn attributes(&self) -> Vec<DynamicAttribute> {
        match self {
            Self::Queueing { operator_id } => vec![DynamicAttribute::string(
                "operator_id",
                operator_id.target.to_string(),
            )],
            Self::Computing { input_bytes, .. } => {
                vec![DynamicAttribute::u64("input_bytes", *input_bytes)]
            }
            _ => vec![],
        }
    }

    fn parent_group_id(&self) -> Option<Uuid> {
        None
    }

    fn fsm_type_name() -> &'static str {
        "task"
    }

    fn collect_model(builder: &mut ModelBuilder) {
        fn state(name: &str, usages: &[(&str, &str)]) -> StateDef {
            StateDef {
                name: name.to_owned(),
                attributes: vec![],
                usages: usages
                    .iter()
                    .map(|(field_name, resource_name)| UsageDef {
                        field_name: (*field_name).to_owned(),
                        resource_name: (*resource_name).to_owned(),
                        resource_type_path: String::new(),
                    })
                    .collect(),
            }
        }

        fn endpoint(name: &str) -> TransitionEndpoint {
            TransitionEndpoint::State(name.to_owned())
        }

        fn transition(from: &str, to: &str) -> TransitionDef {
            TransitionDef {
                from: endpoint(from),
                to: endpoint(to),
            }
        }

        builder.add_fsm(FsmDef {
            name: "task".to_owned(),
            module_path: module_path!().to_owned(),
            entry: "queueing".to_owned(),
            states: vec![
                state("queueing", &[]),
                state("allocating", &[("use_thread", "processor")]),
                state(
                    "loading",
                    &[
                        ("use_thread", "processor"),
                        ("use_fs_to_mem", "storage_channel"),
                        ("use_memory", "memory"),
                    ],
                ),
                state(
                    "computing",
                    &[("use_thread", "processor"), ("use_memory", "memory")],
                ),
                state(
                    "spilling",
                    &[
                        ("use_thread", "processor"),
                        ("use_mem_to_fs", "storage_channel"),
                    ],
                ),
                state(
                    "sending",
                    &[("use_thread", "processor"), ("use_link", "network_channel")],
                ),
            ],
            transitions: vec![
                TransitionDef {
                    from: TransitionEndpoint::Entry,
                    to: endpoint("queueing"),
                },
                transition("queueing", "allocating"),
                transition("allocating", "computing"),
                transition("allocating", "loading"),
                transition("loading", "computing"),
                transition("computing", "sending"),
                transition("computing", "spilling"),
                TransitionDef {
                    from: endpoint("computing"),
                    to: TransitionEndpoint::Exit,
                },
                transition("spilling", "allocating"),
                transition("sending", "queueing"),
            ],
        });
    }
}

/// The reconstructed Task FSM.
pub type Task = FsmEvents<TaskTransition>;

/// Builder for Task FSMs.
pub type TaskBuilder = FsmEventsBuilder<TaskTransition>;

/// Application-specific methods on the Task FSM.
pub trait TaskExt {
    fn operator_id(&self) -> Option<Uuid>;
    fn active_span(&self) -> Option<SpanUnixNanoSec>;
    fn try_to_ui_fsm(&self, epoch: TimeUnixNanoSec) -> AnalyzerResult<FiniteStateMachine>;
}

impl TaskExt for Task {
    fn operator_id(&self) -> Option<Uuid> {
        self.first_data().and_then(|transition| match transition {
            TaskTransition::Queueing { operator_id } => Some(operator_id.target),
            _ => None,
        })
    }

    fn active_span(&self) -> Option<SpanUnixNanoSec> {
        let start = self.transitions().get(1)?.timestamp();
        let end = self.transitions().last()?.timestamp();
        SpanUnixNanoSec::try_new(start, end).ok()
    }

    fn try_to_ui_fsm(&self, epoch: TimeUnixNanoSec) -> AnalyzerResult<FiniteStateMachine> {
        let raw = self.transitions();
        let transitions = raw
            .iter()
            .enumerate()
            .map(|(i, transition)| {
                let mut derived_attributes = vec![];
                if let TaskTransition::Computing { input_bytes, .. } = &transition.data
                    && let Some(next) = raw.get(i + 1)
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
