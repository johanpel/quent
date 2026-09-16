// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Temporary analyzed types for generated schema entities.

// TODO(johanpel): Generate this module from schema metadata. See
// https://github.com/rapidsai/quent/issues/288.

use quent_analyzer::{
    AnalyzerError, AnalyzerResult, Entity, RefTreeEntity,
    entity::native::{AnalyzedEntity, EntityEventAccumulator},
    fsm::{
        Fsm,
        native::{AnalyzedFsm, AnalyzedTransition, FsmBuilder},
    },
    resource::{CapacityDecl, Resource, ResourceGroup, ResourceTypeDecl},
};
use quent_events::Event;
use quent_simulator_store as schema;
use quent_time::TimeUnixNanoSec;
use uuid::Uuid;

#[derive(Default)]
pub(crate) struct TaskExecutorAccumulator {
    instance_name: Option<String>,
    worker_id: Option<Uuid>,
}

impl quent_events::Entity for TaskExecutorAccumulator {
    type Event = schema::TaskExecutorEvent;
}

impl EntityEventAccumulator for TaskExecutorAccumulator {
    fn push(&mut self, event: Self::Event) {
        let schema::TaskExecutorEvent::Declaration {
            instance_name,
            worker_id,
        } = event;
        self.instance_name = Some(instance_name);
        self.worker_id = Some(worker_id.target);
    }
}

pub(crate) struct TaskExecutor(AnalyzedEntity<TaskExecutorAccumulator>);

impl TaskExecutor {
    pub(crate) fn try_from_event(event: Event<schema::TaskExecutorEvent>) -> AnalyzerResult<Self> {
        Ok(Self(AnalyzedEntity::try_from_first_event(event)?))
    }

    pub(crate) fn push(&mut self, event: Event<schema::TaskExecutorEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }
}

impl Entity for TaskExecutor {
    fn id(&self) -> Uuid {
        self.0.id()
    }

    fn type_name(&self) -> &str {
        self.0.type_name()
    }

    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.0.earliest_timestamp()
    }

    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.0.latest_timestamp()
    }
}

impl RefTreeEntity for TaskExecutor {
    fn parent_id(&self) -> Option<Uuid> {
        self.0.accumulator().worker_id
    }
}

impl ResourceGroup for TaskExecutor {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_id()
    }
}

#[derive(Default)]
pub(crate) struct NetworkAccumulator {
    instance_name: Option<String>,
    engine_id: Option<Uuid>,
}

impl quent_events::Entity for NetworkAccumulator {
    type Event = schema::NetworkEvent;
}

impl EntityEventAccumulator for NetworkAccumulator {
    fn push(&mut self, event: Self::Event) {
        let schema::NetworkEvent::Declaration {
            instance_name,
            engine_id,
        } = event;
        self.instance_name = Some(instance_name);
        self.engine_id = Some(engine_id.target);
    }
}

pub(crate) struct Network(AnalyzedEntity<NetworkAccumulator>);

impl Network {
    pub(crate) fn try_from_event(event: Event<schema::NetworkEvent>) -> AnalyzerResult<Self> {
        Ok(Self(AnalyzedEntity::try_from_first_event(event)?))
    }

    pub(crate) fn push(&mut self, event: Event<schema::NetworkEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }
}

impl Entity for Network {
    fn id(&self) -> Uuid {
        self.0.id()
    }

    fn type_name(&self) -> &str {
        self.0.type_name()
    }

    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.0.earliest_timestamp()
    }

    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.0.latest_timestamp()
    }
}

impl RefTreeEntity for Network {
    fn parent_id(&self) -> Option<Uuid> {
        self.0.accumulator().engine_id
    }
}

impl ResourceGroup for Network {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_id()
    }
}

#[derive(Default)]
pub(crate) struct GpuAccumulator {
    instance_name: Option<String>,
    worker_id: Option<Uuid>,
}

impl quent_events::Entity for GpuAccumulator {
    type Event = schema::GpuEvent;
}

impl EntityEventAccumulator for GpuAccumulator {
    fn push(&mut self, event: Self::Event) {
        let schema::GpuEvent::Declaration {
            instance_name,
            worker_id,
        } = event;
        self.instance_name = Some(instance_name);
        self.worker_id = Some(worker_id.target);
    }
}

pub(crate) struct Gpu(AnalyzedEntity<GpuAccumulator>);

impl Gpu {
    pub(crate) fn try_from_event(event: Event<schema::GpuEvent>) -> AnalyzerResult<Self> {
        Ok(Self(AnalyzedEntity::try_from_first_event(event)?))
    }

    pub(crate) fn push(&mut self, event: Event<schema::GpuEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }
}

impl Entity for Gpu {
    fn id(&self) -> Uuid {
        self.0.id()
    }

    fn type_name(&self) -> &str {
        self.0.type_name()
    }

    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.0.earliest_timestamp()
    }

    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.0.latest_timestamp()
    }
}

impl RefTreeEntity for Gpu {
    fn parent_id(&self) -> Option<Uuid> {
        self.0.accumulator().worker_id
    }
}

impl ResourceGroup for Gpu {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_id()
    }
}

pub(crate) struct HostMemoryBuilder {
    fsm: FsmBuilder<schema::HostMemoryEvent>,
    instance_name: Option<String>,
    worker_id: Option<Uuid>,
}

impl HostMemoryBuilder {
    pub(crate) fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self {
            fsm: FsmBuilder::try_new(id)?,
            instance_name: None,
            worker_id: None,
        })
    }

    pub(crate) fn push(&mut self, event: Event<schema::HostMemoryEvent>) {
        if let schema::HostMemoryEvent::Initializing {
            instance_name,
            worker_id,
            ..
        } = &event.data
        {
            self.instance_name = Some(instance_name.clone());
            self.worker_id = Some(worker_id.target);
        }
        self.fsm.push_transition(event);
    }

    pub(crate) fn try_build(self) -> AnalyzerResult<HostMemory> {
        let id = self.fsm.id();
        Ok(HostMemory {
            fsm: self.fsm.try_build()?,
            instance_name: self.instance_name.ok_or_else(|| {
                AnalyzerError::IncompleteEntity(format!(
                    "host memory {id} has no initializing event"
                ))
            })?,
            worker_id: self.worker_id.ok_or_else(|| {
                AnalyzerError::IncompleteEntity(format!(
                    "host memory {id} has no initializing event"
                ))
            })?,
        })
    }
}

pub(crate) struct HostMemory {
    fsm: AnalyzedFsm<schema::HostMemoryEvent>,
    instance_name: String,
    worker_id: Uuid,
}

impl HostMemory {
    pub(crate) fn instance_name(&self) -> &str {
        &self.instance_name
    }

    pub(crate) fn resource_type_decl() -> ResourceTypeDecl {
        ResourceTypeDecl::new("host_memory", [CapacityDecl::new_occupancy("bytes")])
    }
}

impl Entity for HostMemory {
    fn id(&self) -> Uuid {
        self.fsm.id()
    }

    fn type_name(&self) -> &str {
        "host_memory"
    }

    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.fsm.earliest_timestamp()
    }

    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.fsm.latest_timestamp()
    }
}

impl Fsm for HostMemory {
    type TransitionType = AnalyzedTransition<schema::HostMemoryEvent>;

    fn len(&self) -> usize {
        self.fsm.len()
    }

    fn transition(&self, index: usize) -> Option<&Self::TransitionType> {
        self.fsm.transition(index)
    }
}

impl Resource for HostMemory {
    fn parent_group_id(&self) -> Uuid {
        self.worker_id
    }
}

impl RefTreeEntity for HostMemory {
    fn parent_id(&self) -> Option<Uuid> {
        Some(self.worker_id)
    }
}

#[cfg(test)]
mod tests {
    use quent_analyzer::fsm::Fsm as _;
    use quent_events::EntityRef;

    use super::*;

    #[test]
    fn builds_host_memory_from_schema_events() {
        let id = Uuid::from_u128(1);
        let worker_id = Uuid::from_u128(2);
        let mut builder = HostMemoryBuilder::try_new(id).unwrap();
        builder.push(Event::new(
            id,
            10,
            schema::HostMemoryEvent::Initializing {
                seq: 0,
                instance_name: "memory".to_owned(),
                worker_id: EntityRef::new(worker_id, ()),
            },
        ));
        builder.push(Event::new(
            id,
            20,
            schema::HostMemoryEvent::Operating { seq: 1 },
        ));
        builder.push(Event::new(
            id,
            30,
            schema::HostMemoryEvent::Finalizing { seq: 2 },
        ));
        builder.push(Event::new(id, 40, schema::HostMemoryEvent::Exit { seq: 3 }));

        let resource = builder.try_build().unwrap();

        assert_eq!(resource.id(), id);
        assert_eq!(resource.type_name(), "host_memory");
        assert_eq!(resource.instance_name(), "memory");
        assert_eq!(resource.parent_group_id(), worker_id);
        assert_eq!(resource.parent_id(), Some(worker_id));
        assert_eq!(resource.earliest_timestamp(), 10);
        assert_eq!(resource.latest_timestamp(), 40);
        assert_eq!(resource.len(), 3);
    }
}
