// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use rustc_hash::FxHashMap as HashMap;

use quent_analyzer::{
    AnalyzerError, AnalyzerResult, Entity, Model,
    fsm::collection::FsmCollection,
    resource::{
        CapacityValue, Resource, ResourceCapacities, ResourceGroup, ResourceGroupTypeDecl,
        ResourceTypeDecl, Usage, Using,
        collection::{
            InMemoryResources, InMemoryResourcesBuilder, ResourceCollection,
            derive_resource_group_types,
        },
        runtime::RtResourceTransition,
    },
};
use quent_events::Event;
use quent_model::{FsmEvent, Ref};
use quent_query_engine_analyzer::{
    OperatorEntityMut, QueryEngineModel, QueryEngineModelMut,
    plain::legacy::{
        Engine, InMemoryQueryEngineModel, InMemoryQueryEngineModelBuilder, Operator, Plan, Port,
        Query, QueryEngineEntityId, QueryGroup, Worker,
    },
    plan_tree::PlanTree,
};
use quent_query_engine_model::{self as qe, QueryEngineEvent};
use quent_simulator_instrumentation::{self as instr, SimulatorEvent};
use quent_simulator_ui::EntityRef;
use uuid::Uuid;

use crate::{
    task::{Task, TaskBuilder, TaskExt},
    view::SimulatorModelQueryView,
};

/// A model of the simulator engine
pub struct SimulatorModel {
    pub(crate) query_engine: InMemoryQueryEngineModel,
    pub(crate) arbitrary_resources: InMemoryResources,
    pub(crate) tasks: HashMap<Uuid, Task>,
    pub(crate) resource_group_types: HashMap<String, ResourceGroupTypeDecl>,
}

impl Model for SimulatorModel {
    type EntityIdType = EntityRef;

    fn try_entity_ref(&self, entity_id: Uuid) -> AnalyzerResult<Self::EntityIdType> {
        if let Ok(qe_ref) = self.query_engine.try_entity_ref(entity_id) {
            Ok(match qe_ref {
                QueryEngineEntityId::Engine(uuid) => EntityRef::Engine(uuid),
                QueryEngineEntityId::Worker(uuid) => EntityRef::Worker(uuid),
                QueryEngineEntityId::QueryGroup(uuid) => EntityRef::QueryGroup(uuid),
                QueryEngineEntityId::Query(uuid) => EntityRef::Query(uuid),
                QueryEngineEntityId::Plan(uuid) => EntityRef::Plan(uuid),
                QueryEngineEntityId::Operator(uuid) => EntityRef::Operator(uuid),
                QueryEngineEntityId::Port(uuid) => EntityRef::Port(uuid),
            })
        } else if self.arbitrary_resources.resources.contains_key(&entity_id) {
            Ok(EntityRef::Resource(entity_id))
        } else if self
            .arbitrary_resources
            .resource_groups
            .contains_key(&entity_id)
        {
            Ok(EntityRef::ResourceGroup(entity_id))
        } else {
            self.tasks
                .contains_key(&entity_id)
                .then_some(EntityRef::Task(entity_id))
                .ok_or(AnalyzerError::InvalidId(entity_id))
        }
    }

    fn root(&self) -> AnalyzerResult<&impl ResourceGroup> {
        self.query_engine.root()
    }
}

impl QueryEngineModel for SimulatorModel {
    type Engine = Engine;
    type Query = Query;
    type QueryGroup = QueryGroup;
    type Worker = Worker;
    type Plan = Plan;
    type Operator = Operator;
    type Port = Port;

    fn engine(&self) -> AnalyzerResult<&Engine> {
        self.query_engine.engine()
    }
    fn query(&self, query_id: Uuid) -> AnalyzerResult<&Query> {
        self.query_engine.query(query_id)
    }
    fn query_group(&self, query_group_id: Uuid) -> AnalyzerResult<&QueryGroup> {
        self.query_engine.query_group(query_group_id)
    }
    fn worker(&self, worker_id: Uuid) -> AnalyzerResult<&Worker> {
        self.query_engine.worker(worker_id)
    }
    fn plan(&self, plan_id: Uuid) -> AnalyzerResult<&Plan> {
        self.query_engine.plan(plan_id)
    }
    fn operator(&self, operator_id: Uuid) -> AnalyzerResult<&Operator> {
        self.query_engine.operator(operator_id)
    }
    fn port(&self, port_id: Uuid) -> AnalyzerResult<&Port> {
        self.query_engine.port(port_id)
    }
    fn queries(&self) -> impl Iterator<Item = &Query> {
        self.query_engine.queries()
    }
    fn query_groups(&self) -> impl Iterator<Item = &QueryGroup> {
        self.query_engine.query_groups()
    }
    fn workers(&self) -> impl Iterator<Item = &Worker> {
        self.query_engine.workers()
    }
    fn plans(&self) -> impl Iterator<Item = &Plan> {
        self.query_engine.plans()
    }
    fn operators(&self) -> impl Iterator<Item = &Operator> {
        self.query_engine.operators()
    }
    fn ports(&self) -> impl Iterator<Item = &Port> {
        self.query_engine.ports()
    }
    fn plan_tree(&self, query_id: Uuid) -> AnalyzerResult<PlanTree> {
        self.query_engine.plan_tree(query_id)
    }
}

impl QueryEngineModelMut for SimulatorModel {
    fn operator_mut(&mut self, operator_id: Uuid) -> AnalyzerResult<&mut Operator> {
        self.query_engine.operator_mut(operator_id)
    }
}

impl SimulatorModel {
    pub(crate) fn query_view(&self, query_id: Uuid) -> AnalyzerResult<SimulatorModelQueryView<'_>> {
        SimulatorModelQueryView::try_new(self, query_id)
    }
}

impl FsmCollection for SimulatorModel {
    type Fsm = Task;

    fn fsms(&self) -> impl Iterator<Item = &Task> {
        self.tasks.values()
    }
}

impl ResourceCollection for SimulatorModel {
    fn resources(&self) -> impl Iterator<Item = &dyn Resource> {
        self.arbitrary_resources
            .resources()
            .chain(self.query_engine.resources())
    }
    fn resource_groups(&self) -> impl Iterator<Item = &dyn ResourceGroup> {
        self.arbitrary_resources
            .resource_groups()
            .chain(self.query_engine.resource_groups())
    }
    fn resource(&self, resource_id: Uuid) -> AnalyzerResult<&dyn Resource> {
        self.arbitrary_resources
            .resource(resource_id)
            .or_else(|_| self.query_engine.resource(resource_id))
    }
    fn resource_type(&self, resource_type_name: &str) -> AnalyzerResult<&ResourceTypeDecl> {
        self.query_engine
            .resource_type(resource_type_name)
            .or_else(|_| self.arbitrary_resources.resource_type(resource_type_name))
    }
    fn resource_group(&self, resource_group_id: Uuid) -> AnalyzerResult<&dyn ResourceGroup> {
        self.query_engine
            .resource_group(resource_group_id)
            .or_else(|_| self.arbitrary_resources.resource_group(resource_group_id))
    }

    fn resource_group_child_groups(
        &self,
        resource_group_id: Uuid,
    ) -> AnalyzerResult<impl Iterator<Item = Uuid>> {
        // Verify the resource group exists in at least one collection
        self.resource_group(resource_group_id)?;

        let engine = self
            .query_engine
            .resource_group_child_groups(resource_group_id)
            .ok();

        let sim = self
            .arbitrary_resources
            .resource_groups
            .values()
            .filter_map(move |group| {
                group
                    .parent_group_id
                    .and_then(|parent| (parent == resource_group_id).then_some(group.id))
            });

        Ok(engine.into_iter().flatten().chain(sim))
    }

    fn resource_group_child_resources(
        &self,
        resource_group_id: Uuid,
    ) -> AnalyzerResult<impl Iterator<Item = Uuid>> {
        // Verify the resource group exists in at least one collection
        self.resource_group(resource_group_id)?;

        let engine = self
            .query_engine
            .resource_group_child_resources(resource_group_id)
            .ok();

        let sim = self
            .arbitrary_resources
            .resources
            .values()
            .filter_map(move |resource| {
                (resource.parent_group_id() == resource_group_id).then_some(resource.id)
            });

        Ok(engine.into_iter().flatten().chain(sim))
    }
}

impl Using for SimulatorModel {
    fn usages(&self) -> impl Iterator<Item = impl Usage<'_>> {
        self.tasks.values().flat_map(|task| task.usages())
    }
}

fn engine_event(event: instr::EngineEvent) -> qe::engine::EngineEvent {
    match event {
        instr::EngineEvent::Init {
            implementation,
            instance_name,
        } => qe::engine::EngineEvent::Init(qe::engine::Init {
            implementation: qe::engine::EngineImplementationAttributes {
                name: implementation.name,
                version: implementation.version,
                custom_attributes: implementation.custom_attributes,
            },
            instance_name,
        }),
        instr::EngineEvent::Exit => qe::engine::EngineEvent::Exit(qe::engine::Exit),
    }
}

fn worker_event(event: instr::WorkerEvent) -> qe::worker::WorkerEvent {
    match event {
        instr::WorkerEvent::Init {
            parent_engine_id,
            instance_name,
        } => qe::worker::WorkerEvent::Init(qe::worker::Init {
            parent_engine_id: Ref::new(parent_engine_id.target),
            instance_name,
        }),
        instr::WorkerEvent::Exit => qe::worker::WorkerEvent::Exit(qe::worker::Exit),
    }
}

fn query_group_event(event: instr::QueryGroupEvent) -> qe::query_group::QueryGroupEvent {
    match event {
        instr::QueryGroupEvent::Declaration {
            instance_name,
            engine_id,
        } => qe::query_group::QueryGroupEvent::Declaration(qe::query_group::Declaration {
            instance_name,
            engine_id: engine_id.target,
        }),
    }
}

fn query_event(event: instr::QueryEvent) -> qe::query::QueryEvent {
    let state = match event {
        instr::QueryEvent::Init {
            instance_name,
            query_group_id,
        } => qe::query::QueryTransition::Init(qe::query::Init {
            query_group_id: Ref::new(query_group_id.target),
            instance_name,
        }),
        instr::QueryEvent::Planning => qe::query::QueryTransition::Planning(qe::query::Planning {}),
        instr::QueryEvent::Executing => {
            qe::query::QueryTransition::Executing(qe::query::Executing {})
        }
        instr::QueryEvent::Exit => qe::query::QueryTransition::Exit,
    };
    FsmEvent { seq: 0, state }
}

fn plan_event(event: instr::PlanEvent) -> qe::plan::PlanEvent {
    match event {
        instr::PlanEvent::Declaration {
            parent,
            instance_name,
            edges,
            worker_id,
        } => qe::plan::PlanEvent::Declaration(qe::plan::Declaration {
            parent: qe::plan::PlanParent {
                query_id: parent
                    .plan_id
                    .is_none()
                    .then(|| Ref::new(parent.query_id.target)),
                plan_id: parent.plan_id.map(|plan| Ref::new(plan.target)),
            },
            instance_name,
            edges: edges
                .into_iter()
                .map(|edge| qe::plan::Edge {
                    source: Ref::new(edge.source.target),
                    target: Ref::new(edge.target.target),
                })
                .collect(),
            worker_id: worker_id.map(|worker| Ref::new(worker.target)),
        }),
    }
}

fn operator_event(event: instr::OperatorEvent) -> qe::operator::OperatorEvent {
    match event {
        instr::OperatorEvent::Declaration {
            plan_id,
            parent_operator_ids,
            instance_name,
            type_name,
            custom_attributes,
        } => qe::operator::OperatorEvent::Declaration(qe::operator::Declaration {
            plan_id: Ref::new(plan_id.target),
            parent_operator_ids: parent_operator_ids
                .into_iter()
                .map(|operator| Ref::new(operator.target))
                .collect(),
            instance_name,
            type_name,
            custom_attributes,
        }),
        instr::OperatorEvent::Statistics { custom_attributes } => {
            qe::operator::OperatorEvent::Statistics(qe::operator::Statistics { custom_attributes })
        }
    }
}

fn port_event(event: instr::PortEvent) -> qe::port::PortEvent {
    match event {
        instr::PortEvent::Declaration {
            operator_id,
            instance_name,
        } => qe::port::PortEvent::Declaration(qe::port::Declaration {
            operator_id: Ref::new(operator_id.target),
            instance_name,
        }),
        instr::PortEvent::Statistics { custom_attributes } => {
            qe::port::PortEvent::Statistics(qe::port::Statistics { custom_attributes })
        }
    }
}

pub struct SimulatorModelBuilder {
    query_engine: InMemoryQueryEngineModelBuilder,
    arbitrary_resources: InMemoryResourcesBuilder,
    tasks: HashMap<Uuid, TaskBuilder>,
}

impl SimulatorModelBuilder {
    pub(crate) fn try_new(engine_id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self {
            query_engine: InMemoryQueryEngineModelBuilder::try_new(engine_id)?,
            arbitrary_resources: InMemoryResourcesBuilder::default(),
            tasks: HashMap::default(),
        })
    }

    pub(crate) fn try_push(&mut self, event: Event<SimulatorEvent>) -> AnalyzerResult<()> {
        let Event {
            id,
            timestamp,
            data,
        } = event;
        match data {
            SimulatorEvent::Task(t) => {
                let task_builder = self
                    .tasks
                    .entry(id)
                    .or_insert_with(|| TaskBuilder::try_new(id).unwrap());
                task_builder.push(Event::new(
                    id,
                    timestamp,
                    FsmEvent {
                        seq: 0,
                        state: t.into(),
                    },
                ));
                Ok(())
            }
            SimulatorEvent::Engine(e) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                QueryEngineEvent::Engine(engine_event(e)),
            )),
            SimulatorEvent::Worker(e) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                QueryEngineEvent::Worker(worker_event(e)),
            )),
            SimulatorEvent::QueryGroup(e) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                QueryEngineEvent::QueryGroup(query_group_event(e)),
            )),
            SimulatorEvent::Query(e) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                QueryEngineEvent::Query(query_event(e)),
            )),
            SimulatorEvent::Plan(e) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                QueryEngineEvent::Plan(plan_event(e)),
            )),
            SimulatorEvent::Operator(e) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                QueryEngineEvent::Operator(operator_event(e)),
            )),
            SimulatorEvent::Port(e) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                QueryEngineEvent::Port(port_event(e)),
            )),
            SimulatorEvent::Memory(m) => self.push_memory(id, timestamp, m),
            SimulatorEvent::Processor(p) => self.push_processor(id, timestamp, p),
            SimulatorEvent::StorageChannel(c) => self.push_storage_channel(id, timestamp, c),
            SimulatorEvent::NetworkChannel(c) => self.push_network_channel(id, timestamp, c),
            SimulatorEvent::ThreadPool(instr::ThreadPoolEvent::Declaration {
                instance_name,
                worker_id,
            }) => {
                self.arbitrary_resources.push_group_raw(
                    id,
                    "thread_pool",
                    &instance_name,
                    Some(worker_id.target),
                );
                Ok(())
            }
            SimulatorEvent::Network(instr::NetworkEvent::Declaration {
                instance_name,
                engine_id,
            }) => {
                self.arbitrary_resources.push_group_raw(
                    id,
                    "network",
                    &instance_name,
                    Some(engine_id.target),
                );
                Ok(())
            }
        }
    }

    fn push_memory(
        &mut self,
        id: Uuid,
        timestamp: quent_time::TimeUnixNanoSec,
        event: instr::MemoryEvent,
    ) -> AnalyzerResult<()> {
        match event {
            instr::MemoryEvent::Initializing {
                instance_name,
                worker_id,
            } => {
                self.arbitrary_resources.insert_memory_resource("memory");
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Init(timestamp));
                bld.set_type_name("memory".to_owned());
                bld.set_instance_name(Some(instance_name));
                bld.set_parent_group_id(worker_id.target);
            }
            instr::MemoryEvent::Operating { limits } => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Operating(
                    timestamp,
                    ResourceCapacities(vec![CapacityValue::new("capacity_bytes", limits.bytes)]),
                ));
            }
            instr::MemoryEvent::Finalizing => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Finalizing(timestamp));
            }
            instr::MemoryEvent::Exit => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Exit(timestamp));
            }
        }
        Ok(())
    }

    fn push_processor(
        &mut self,
        id: Uuid,
        timestamp: quent_time::TimeUnixNanoSec,
        event: instr::ProcessorEvent,
    ) -> AnalyzerResult<()> {
        match event {
            instr::ProcessorEvent::Initializing {
                instance_name,
                thread_pool_id,
            } => {
                self.arbitrary_resources
                    .insert_processor_resource("processor");
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Init(timestamp));
                bld.set_type_name("processor".to_owned());
                bld.set_instance_name(Some(instance_name));
                bld.set_parent_group_id(thread_pool_id.target);
            }
            instr::ProcessorEvent::Operating => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Operating(
                    timestamp,
                    ResourceCapacities(vec![]),
                ));
            }
            instr::ProcessorEvent::Finalizing => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Finalizing(timestamp));
            }
            instr::ProcessorEvent::Exit => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Exit(timestamp));
            }
        }
        Ok(())
    }

    fn push_storage_channel(
        &mut self,
        id: Uuid,
        timestamp: quent_time::TimeUnixNanoSec,
        event: instr::StorageChannelEvent,
    ) -> AnalyzerResult<()> {
        match event {
            instr::StorageChannelEvent::Initializing {
                instance_name,
                worker_id,
                ..
            } => {
                self.arbitrary_resources.insert_channel_resource("channel");
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Init(timestamp));
                bld.set_type_name("channel".to_owned());
                bld.set_instance_name(Some(instance_name));
                bld.set_parent_group_id(worker_id.target);
            }
            instr::StorageChannelEvent::Operating { .. } => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Operating(
                    timestamp,
                    ResourceCapacities(vec![]),
                ));
            }
            instr::StorageChannelEvent::Finalizing => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Finalizing(timestamp));
            }
            instr::StorageChannelEvent::Exit => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Exit(timestamp));
            }
        }
        Ok(())
    }

    fn push_network_channel(
        &mut self,
        id: Uuid,
        timestamp: quent_time::TimeUnixNanoSec,
        event: instr::NetworkChannelEvent,
    ) -> AnalyzerResult<()> {
        match event {
            instr::NetworkChannelEvent::Initializing {
                instance_name,
                network_id,
                ..
            } => {
                self.arbitrary_resources.insert_channel_resource("channel");
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Init(timestamp));
                bld.set_type_name("channel".to_owned());
                bld.set_instance_name(Some(instance_name));
                bld.set_parent_group_id(network_id.target);
            }
            instr::NetworkChannelEvent::Operating { .. } => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Operating(
                    timestamp,
                    ResourceCapacities(vec![]),
                ));
            }
            instr::NetworkChannelEvent::Finalizing => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Finalizing(timestamp));
            }
            instr::NetworkChannelEvent::Exit => {
                let bld = self.arbitrary_resources.try_builder(id)?;
                bld.push(RtResourceTransition::Exit(timestamp));
            }
        }
        Ok(())
    }

    pub(crate) fn try_build(self) -> AnalyzerResult<SimulatorModel> {
        // Build resources first. As we iterate over task builders and build all
        // tasks, we can populate the leaf resources used_by field.
        let mut resources = self.arbitrary_resources.try_build()?;
        let mut query_engine = self.query_engine.try_build()?;

        let mut tasks = HashMap::default();

        for (task_id, task_builder) in self.tasks.into_iter() {
            let task = task_builder.try_build()?;
            for usage in task.usages() {
                let resource_type_name = resources
                    .resource(usage.resource_id())?
                    .type_name()
                    .to_owned();
                let set = &mut resources
                    .resource_types
                    .get_mut(&resource_type_name)
                    .unwrap()
                    .used_by;
                if !set.contains(task.type_name()) {
                    set.insert(task.type_name().to_owned());
                }
            }
            if let Some(operator_id) = task.operator_id()
                && let Some(task_span) = task.active_span()
                && let Ok(operator) = query_engine.operator_mut(operator_id)
            {
                operator.extend_active_span(task_span);
            }

            tasks.insert(task_id, task);
        }

        // Construct the model without group type decls being populated yet, we
        // will populate it based on the resource tree.
        let temp_model = SimulatorModel {
            query_engine,
            arbitrary_resources: resources,
            tasks,
            resource_group_types: HashMap::default(),
        };
        let mut resource_group_types = derive_resource_group_types(&temp_model)?;
        // Bubble up all the used_by_entity fields in the group type decls.
        for group_type_decl in resource_group_types.values_mut() {
            for contained_resource_type in &group_type_decl.contains_resource_types {
                if let Ok(resource_type) = temp_model
                    .arbitrary_resources
                    .resource_type(contained_resource_type)
                {
                    for entity_type in &resource_type.used_by {
                        group_type_decl
                            .used_by_entity_types
                            .insert(entity_type.clone());
                    }
                }
            }
        }

        Ok(SimulatorModel {
            query_engine: temp_model.query_engine,
            arbitrary_resources: temp_model.arbitrary_resources,
            tasks: temp_model.tasks,
            resource_group_types,
        })
    }
}
