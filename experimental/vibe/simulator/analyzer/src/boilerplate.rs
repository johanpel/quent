// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Temporary analyzed types for generated schema entities.

// TODO(johanpel): Generate this module from schema metadata. See
// https://github.com/rapidsai/quent/issues/288.

use quent_analyzer::{
    AnalyzerError, AnalyzerResult, Entity, RefTreeEntity,
    entity::native::{AnalyzedEntity, EntityEventAccumulator},
    fsm::{
        Fsm, FsmUsages,
        native::{AnalyzedFsm, AnalyzedTransition, FsmBuilder},
    },
    resource::{CapacityDecl, Resource, ResourceGroup, ResourceTypeDecl, Usage, Using},
};
use quent_events::Event;
use quent_simulator_store as schema;
use quent_time::{TimeUnixNanoSec, span::SpanUnixNanoSec};
use uuid::Uuid;

#[derive(Default)]
pub(crate) struct EngineAccumulator {
    pub(crate) instance_name: Option<String>,
    pub(crate) implementation: Option<schema::EngineImplementationAttributes>,
}

impl quent_events::Entity for EngineAccumulator {
    type Event = schema::EngineEvent;
}

impl EntityEventAccumulator for EngineAccumulator {
    fn push(&mut self, event: Self::Event) {
        if let schema::EngineEvent::Init {
            implementation,
            instance_name,
        } = event
        {
            self.instance_name = instance_name;
            self.implementation = Some(implementation);
        }
    }
}

#[derive(Debug)]
pub(crate) struct Engine(AnalyzedEntity<EngineAccumulator>);

impl Engine {
    pub(crate) fn try_from_event(event: Event<schema::EngineEvent>) -> AnalyzerResult<Self> {
        Ok(Self(AnalyzedEntity::try_from_first_event(event)?))
    }

    pub(crate) fn push(&mut self, event: Event<schema::EngineEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }

    pub(crate) fn data(&self) -> &EngineAccumulator {
        self.0.accumulator()
    }
}

impl Entity for Engine {
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

impl RefTreeEntity for Engine {
    fn parent_id(&self) -> Option<Uuid> {
        None
    }
}

impl ResourceGroup for Engine {
    fn parent_group_id(&self) -> Option<Uuid> {
        None
    }
}

#[derive(Default)]
pub(crate) struct WorkerAccumulator {
    pub(crate) parent_engine_id: Option<Uuid>,
    pub(crate) instance_name: Option<String>,
}

impl quent_events::Entity for WorkerAccumulator {
    type Event = schema::WorkerEvent;
}

impl EntityEventAccumulator for WorkerAccumulator {
    fn push(&mut self, event: Self::Event) {
        if let schema::WorkerEvent::Init {
            parent_engine_id,
            instance_name,
        } = event
        {
            self.parent_engine_id = Some(parent_engine_id.target);
            self.instance_name = Some(instance_name);
        }
    }
}

#[derive(Debug)]
pub(crate) struct Worker(AnalyzedEntity<WorkerAccumulator>);

impl Worker {
    pub(crate) fn try_from_event(event: Event<schema::WorkerEvent>) -> AnalyzerResult<Self> {
        Ok(Self(AnalyzedEntity::try_from_first_event(event)?))
    }

    pub(crate) fn push(&mut self, event: Event<schema::WorkerEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }

    pub(crate) fn data(&self) -> &WorkerAccumulator {
        self.0.accumulator()
    }
}

impl Entity for Worker {
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

impl RefTreeEntity for Worker {
    fn parent_id(&self) -> Option<Uuid> {
        self.0.accumulator().parent_engine_id
    }
}

impl ResourceGroup for Worker {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_id()
    }
}

#[derive(Default)]
pub(crate) struct QueryGroupAccumulator {
    pub(crate) instance_name: Option<String>,
    pub(crate) engine_id: Option<Uuid>,
}

impl quent_events::Entity for QueryGroupAccumulator {
    type Event = schema::QueryGroupEvent;
}

impl EntityEventAccumulator for QueryGroupAccumulator {
    fn push(&mut self, event: Self::Event) {
        let schema::QueryGroupEvent::Declaration {
            instance_name,
            engine_id,
        } = event;
        self.instance_name = Some(instance_name);
        self.engine_id = Some(engine_id.target);
    }
}

#[derive(Debug)]
pub(crate) struct QueryGroup(AnalyzedEntity<QueryGroupAccumulator>);

impl QueryGroup {
    pub(crate) fn try_from_event(event: Event<schema::QueryGroupEvent>) -> AnalyzerResult<Self> {
        Ok(Self(AnalyzedEntity::try_from_first_event(event)?))
    }

    pub(crate) fn push(&mut self, event: Event<schema::QueryGroupEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }

    pub(crate) fn data(&self) -> &QueryGroupAccumulator {
        self.0.accumulator()
    }
}

impl Entity for QueryGroup {
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

impl RefTreeEntity for QueryGroup {
    fn parent_id(&self) -> Option<Uuid> {
        self.0.accumulator().engine_id
    }
}

impl ResourceGroup for QueryGroup {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_id()
    }
}

pub(crate) type QueryBuilder = FsmBuilder<schema::QueryEvent>;

#[derive(Debug)]
pub(crate) struct Query(AnalyzedFsm<schema::QueryEvent>);

impl Query {
    pub(crate) fn try_from_builder(builder: QueryBuilder) -> AnalyzerResult<Self> {
        Ok(Self(builder.try_build()?))
    }

    pub(crate) fn transitions(&self) -> &[AnalyzedTransition<schema::QueryEvent>] {
        self.0.transitions()
    }

    pub(crate) fn query_group_id(&self) -> Option<Uuid> {
        match self.0.first_data()? {
            schema::QueryEvent::Init { query_group_id, .. } => Some(query_group_id.target),
            _ => None,
        }
    }
}

impl Entity for Query {
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

impl Fsm for Query {
    type TransitionType = AnalyzedTransition<schema::QueryEvent>;

    fn len(&self) -> usize {
        self.0.len()
    }

    fn transition(&self, index: usize) -> Option<&Self::TransitionType> {
        self.0.transition(index)
    }
}

impl<'a> FsmUsages<'a> for Query {
    fn usages_with_state_names(&'a self) -> impl Iterator<Item = (&'a str, impl Usage<'a>)> {
        self.0.usages_with_state_names()
    }
}

impl Using for Query {
    fn usages(&self) -> impl Iterator<Item = impl Usage<'_>> {
        self.0.usages()
    }
}

impl RefTreeEntity for Query {
    fn parent_id(&self) -> Option<Uuid> {
        self.query_group_id()
    }
}

impl ResourceGroup for Query {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_id()
    }
}

#[derive(Default)]
pub(crate) struct PlanAccumulator {
    pub(crate) instance_name: Option<String>,
    pub(crate) parent_query_id: Option<Uuid>,
    pub(crate) parent_plan_id: Option<Uuid>,
    pub(crate) worker_id: Option<Uuid>,
    pub(crate) edges: Vec<(Uuid, Uuid)>,
}

impl quent_events::Entity for PlanAccumulator {
    type Event = schema::PlanEvent;
}

impl EntityEventAccumulator for PlanAccumulator {
    fn push(&mut self, event: Self::Event) {
        let schema::PlanEvent::Declaration {
            parent,
            instance_name,
            edges,
            worker_id,
        } = event;
        self.instance_name = Some(instance_name);
        self.parent_query_id = Some(parent.query_id.target);
        self.parent_plan_id = parent.plan_id.map(|plan| plan.target);
        self.worker_id = worker_id.map(|worker| worker.target);
        self.edges = edges
            .into_iter()
            .map(|edge| (edge.source.target, edge.target.target))
            .collect();
    }
}

#[derive(Debug)]
pub(crate) struct Plan(AnalyzedEntity<PlanAccumulator>);

impl Plan {
    pub(crate) fn try_from_event(event: Event<schema::PlanEvent>) -> AnalyzerResult<Self> {
        Ok(Self(AnalyzedEntity::try_from_first_event(event)?))
    }

    pub(crate) fn push(&mut self, event: Event<schema::PlanEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }

    pub(crate) fn data(&self) -> &PlanAccumulator {
        self.0.accumulator()
    }
}

impl Entity for Plan {
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

impl RefTreeEntity for Plan {
    fn parent_id(&self) -> Option<Uuid> {
        self.0.accumulator().parent_query_id
    }
}

impl ResourceGroup for Plan {
    fn parent_group_id(&self) -> Option<Uuid> {
        let data = self.0.accumulator();
        data.worker_id
            .or(data.parent_plan_id)
            .or(data.parent_query_id)
    }
}

#[derive(Default)]
pub(crate) struct OperatorAccumulator {
    pub(crate) plan_id: Option<Uuid>,
    pub(crate) parent_operator_ids: Vec<Uuid>,
    pub(crate) instance_name: Option<String>,
    pub(crate) type_name: Option<String>,
    pub(crate) custom_attributes: quent_events::DynamicAttributes,
    pub(crate) statistics: Option<quent_events::DynamicAttributes>,
}

impl quent_events::Entity for OperatorAccumulator {
    type Event = schema::OperatorEvent;
}

impl EntityEventAccumulator for OperatorAccumulator {
    fn push(&mut self, event: Self::Event) {
        match event {
            schema::OperatorEvent::Declaration {
                plan_id,
                parent_operator_ids,
                instance_name,
                type_name,
                custom_attributes,
            } => {
                self.plan_id = Some(plan_id.target);
                self.parent_operator_ids = parent_operator_ids
                    .into_iter()
                    .map(|operator| operator.target)
                    .collect();
                self.instance_name = Some(instance_name);
                self.type_name = Some(type_name);
                self.custom_attributes = custom_attributes;
            }
            schema::OperatorEvent::Statistics { custom_attributes } => {
                self.statistics = Some(custom_attributes);
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Operator {
    entity: AnalyzedEntity<OperatorAccumulator>,
    pub(crate) active_span: Option<SpanUnixNanoSec>,
}

impl Operator {
    pub(crate) fn try_from_event(event: Event<schema::OperatorEvent>) -> AnalyzerResult<Self> {
        Ok(Self {
            entity: AnalyzedEntity::try_from_first_event(event)?,
            active_span: None,
        })
    }

    pub(crate) fn push(&mut self, event: Event<schema::OperatorEvent>) -> AnalyzerResult<()> {
        self.entity.push(event)
    }

    pub(crate) fn data(&self) -> &OperatorAccumulator {
        self.entity.accumulator()
    }
}

impl Entity for Operator {
    fn id(&self) -> Uuid {
        self.entity.id()
    }

    fn type_name(&self) -> &str {
        self.entity.type_name()
    }

    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.entity.earliest_timestamp()
    }

    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.entity.latest_timestamp()
    }
}

impl RefTreeEntity for Operator {
    fn parent_id(&self) -> Option<Uuid> {
        self.entity.accumulator().plan_id
    }
}

impl ResourceGroup for Operator {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_id()
    }
}

#[derive(Default)]
pub(crate) struct PortAccumulator {
    pub(crate) operator_id: Option<Uuid>,
    pub(crate) instance_name: Option<String>,
    pub(crate) statistics: Option<quent_events::DynamicAttributes>,
}

impl quent_events::Entity for PortAccumulator {
    type Event = schema::PortEvent;
}

impl EntityEventAccumulator for PortAccumulator {
    fn push(&mut self, event: Self::Event) {
        match event {
            schema::PortEvent::Declaration {
                operator_id,
                instance_name,
            } => {
                self.operator_id = Some(operator_id.target);
                self.instance_name = Some(instance_name);
            }
            schema::PortEvent::Statistics { custom_attributes } => {
                self.statistics = Some(custom_attributes);
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Port(AnalyzedEntity<PortAccumulator>);

impl Port {
    pub(crate) fn try_from_event(event: Event<schema::PortEvent>) -> AnalyzerResult<Self> {
        Ok(Self(AnalyzedEntity::try_from_first_event(event)?))
    }

    pub(crate) fn push(&mut self, event: Event<schema::PortEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }

    pub(crate) fn data(&self) -> &PortAccumulator {
        self.0.accumulator()
    }
}

impl Entity for Port {
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

impl RefTreeEntity for Port {
    fn parent_id(&self) -> Option<Uuid> {
        self.0.accumulator().operator_id
    }
}

impl ResourceGroup for Port {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_id()
    }
}

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
