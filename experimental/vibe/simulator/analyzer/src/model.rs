// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use rustc_hash::FxHashMap as HashMap;

use quent_analyzer::{
    AnalyzerError, AnalyzerResult, Entity, Model, RefTreeEntity,
    fsm::collection::FsmCollection,
    ref_tree::{collection::RefTreeCollection, tree::RefTreeNode},
    resource::{
        Resource, ResourceGroup, ResourceGroupTypeDecl, ResourceTypeDecl, Usage, Using,
        collection::{ResourceCollection, derive_resource_group_types},
    },
};
use quent_events::Event;
use quent_query_engine_analyzer::{
    OperatorEntityMut, QueryEngineEntityId, QueryEngineModel, QueryEngineModelMut,
    model::{
        self as query_engine, Engine, InMemoryQueryEngineModel, InMemoryQueryEngineModelBuilder,
        Operator, Plan, Port, Query, QueryEngineEvent, QueryGroup, Worker,
    },
    plan_tree::PlanTree,
};
use quent_query_engine_ui::EntityRef;
use quent_simulator_store::{self as schema, SimulatorEvent};
use uuid::Uuid;

use crate::{
    boilerplate::{
        Gpu, GpuMemory, HostMemory, Network, NetworkChannel, PcieChannel, Storage, StorageChannel,
        TaskExecutor, TaskExecutorThread,
    },
    task::{Task, TaskBuilder, TaskExt},
    view::SimulatorModelQueryView,
};

trait IntoQueryEngineEvent {
    fn into_query_engine_event(self) -> QueryEngineEvent;
}

// TODO(johanpel): Generate query-engine semantic event adapters from schema metadata. See
// https://github.com/rapidsai/quent/issues/288.
impl IntoQueryEngineEvent for schema::EngineEvent {
    fn into_query_engine_event(self) -> QueryEngineEvent {
        QueryEngineEvent::Engine(match self {
            Self::Init {
                implementation,
                instance_name,
            } => query_engine::EngineEvent::Init {
                implementation: query_engine::EngineImplementation {
                    name: implementation.name,
                    version: implementation.version,
                    custom_attributes: implementation.custom_attributes,
                },
                instance_name,
            },
            Self::Exit => query_engine::EngineEvent::Exit,
        })
    }
}

impl IntoQueryEngineEvent for schema::WorkerEvent {
    fn into_query_engine_event(self) -> QueryEngineEvent {
        QueryEngineEvent::Worker(match self {
            Self::Init {
                parent_engine_id,
                instance_name,
            } => query_engine::WorkerEvent::Init {
                parent_engine_id: parent_engine_id.target,
                instance_name,
            },
            Self::Exit => query_engine::WorkerEvent::Exit,
        })
    }
}

impl IntoQueryEngineEvent for schema::QueryGroupEvent {
    fn into_query_engine_event(self) -> QueryEngineEvent {
        let Self::Declaration {
            instance_name,
            engine_id,
        } = self;
        QueryEngineEvent::QueryGroup(query_engine::QueryGroupEvent {
            instance_name,
            engine_id: engine_id.target,
        })
    }
}

impl IntoQueryEngineEvent for schema::QueryEvent {
    fn into_query_engine_event(self) -> QueryEngineEvent {
        QueryEngineEvent::Query(match self {
            Self::Init {
                seq,
                instance_name,
                query_group_id,
            } => query_engine::QueryEvent::Init {
                seq,
                instance_name,
                query_group_id: query_group_id.target,
            },
            Self::Planning { seq } => query_engine::QueryEvent::Planning { seq },
            Self::Executing { seq } => query_engine::QueryEvent::Executing { seq },
            Self::Done { seq } => query_engine::QueryEvent::Done { seq },
        })
    }
}

impl IntoQueryEngineEvent for schema::PlanEvent {
    fn into_query_engine_event(self) -> QueryEngineEvent {
        let Self::Declaration {
            parent,
            instance_name,
            edges,
            worker_id,
        } = self;
        QueryEngineEvent::Plan(query_engine::PlanEvent {
            parent: query_engine::PlanParent {
                query_id: parent.query_id.target,
                plan_id: parent.plan_id.map(|plan| plan.target),
            },
            instance_name,
            edges: edges
                .into_iter()
                .map(|edge| query_engine::Edge {
                    source: edge.source.target,
                    target: edge.target.target,
                })
                .collect(),
            worker_id: worker_id.map(|worker| worker.target),
        })
    }
}

impl IntoQueryEngineEvent for schema::OperatorEvent {
    fn into_query_engine_event(self) -> QueryEngineEvent {
        QueryEngineEvent::Operator(match self {
            Self::Declaration {
                plan_id,
                parent_operator_ids,
                instance_name,
                type_name,
                custom_attributes,
            } => query_engine::OperatorEvent::Declaration {
                plan_id: plan_id.target,
                parent_operator_ids: parent_operator_ids
                    .into_iter()
                    .map(|operator| operator.target)
                    .collect(),
                instance_name,
                type_name,
                custom_attributes,
            },
            Self::Statistics { custom_attributes } => {
                query_engine::OperatorEvent::Statistics { custom_attributes }
            }
        })
    }
}

impl IntoQueryEngineEvent for schema::PortEvent {
    fn into_query_engine_event(self) -> QueryEngineEvent {
        QueryEngineEvent::Port(match self {
            Self::Declaration {
                operator_id,
                instance_name,
            } => query_engine::PortEvent::Declaration {
                operator_id: operator_id.target,
                instance_name,
            },
            Self::Statistics { custom_attributes } => {
                query_engine::PortEvent::Statistics { custom_attributes }
            }
        })
    }
}

/// A model of the simulator engine
pub struct SimulatorModel {
    pub(crate) query_engine: InMemoryQueryEngineModel,
    pub(crate) resource_types: HashMap<String, ResourceTypeDecl>,
    pub(crate) host_memories: HashMap<Uuid, HostMemory>,
    pub(crate) storages: HashMap<Uuid, Storage>,
    pub(crate) gpu_memories: HashMap<Uuid, GpuMemory>,
    pub(crate) task_executor_threads: HashMap<Uuid, TaskExecutorThread>,
    pub(crate) storage_channels: HashMap<Uuid, StorageChannel>,
    pub(crate) pcie_channels: HashMap<Uuid, PcieChannel>,
    pub(crate) network_channels: HashMap<Uuid, NetworkChannel>,
    pub(crate) task_executors: HashMap<Uuid, TaskExecutor>,
    pub(crate) networks: HashMap<Uuid, Network>,
    pub(crate) gpus: HashMap<Uuid, Gpu>,
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
        } else if self.resource(entity_id).is_ok() {
            Ok(EntityRef::Resource(entity_id))
        } else if self.task_executors.contains_key(&entity_id)
            || self.networks.contains_key(&entity_id)
            || self.gpus.contains_key(&entity_id)
        {
            Ok(EntityRef::ResourceGroup(entity_id))
        } else {
            self.tasks
                .get(&entity_id)
                .map(|task| EntityRef::Application {
                    type_name: task.type_name().to_owned(),
                    id: entity_id,
                })
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

    pub(crate) fn resource_instance_name(&self, resource_id: Uuid) -> Option<&str> {
        self.host_memories
            .get(&resource_id)
            .map(HostMemory::instance_name)
            .or_else(|| self.storages.get(&resource_id).map(Storage::instance_name))
            .or_else(|| {
                self.gpu_memories
                    .get(&resource_id)
                    .map(GpuMemory::instance_name)
            })
            .or_else(|| {
                self.task_executor_threads
                    .get(&resource_id)
                    .map(TaskExecutorThread::instance_name)
            })
            .or_else(|| {
                self.storage_channels
                    .get(&resource_id)
                    .map(StorageChannel::instance_name)
            })
            .or_else(|| {
                self.pcie_channels
                    .get(&resource_id)
                    .map(PcieChannel::instance_name)
            })
            .or_else(|| {
                self.network_channels
                    .get(&resource_id)
                    .map(NetworkChannel::instance_name)
            })
    }

    fn simulator_resource(&self, resource_id: Uuid) -> Option<&dyn Resource> {
        self.host_memories
            .get(&resource_id)
            .map(|resource| resource as &dyn Resource)
            .or_else(|| {
                self.storages
                    .get(&resource_id)
                    .map(|resource| resource as &dyn Resource)
            })
            .or_else(|| {
                self.gpu_memories
                    .get(&resource_id)
                    .map(|resource| resource as &dyn Resource)
            })
            .or_else(|| {
                self.task_executor_threads
                    .get(&resource_id)
                    .map(|resource| resource as &dyn Resource)
            })
            .or_else(|| {
                self.storage_channels
                    .get(&resource_id)
                    .map(|resource| resource as &dyn Resource)
            })
            .or_else(|| {
                self.pcie_channels
                    .get(&resource_id)
                    .map(|resource| resource as &dyn Resource)
            })
            .or_else(|| {
                self.network_channels
                    .get(&resource_id)
                    .map(|resource| resource as &dyn Resource)
            })
    }

    fn simulator_resources(&self) -> impl Iterator<Item = &dyn Resource> {
        self.host_memories
            .values()
            .map(|resource| resource as &dyn Resource)
            .chain(
                self.storages
                    .values()
                    .map(|resource| resource as &dyn Resource),
            )
            .chain(
                self.gpu_memories
                    .values()
                    .map(|resource| resource as &dyn Resource),
            )
            .chain(
                self.task_executor_threads
                    .values()
                    .map(|resource| resource as &dyn Resource),
            )
            .chain(
                self.storage_channels
                    .values()
                    .map(|resource| resource as &dyn Resource),
            )
            .chain(
                self.pcie_channels
                    .values()
                    .map(|resource| resource as &dyn Resource),
            )
            .chain(
                self.network_channels
                    .values()
                    .map(|resource| resource as &dyn Resource),
            )
    }
}

impl FsmCollection for SimulatorModel {
    type Fsm = Task;

    fn fsms(&self) -> impl Iterator<Item = &Task> {
        self.tasks.values()
    }
}

impl RefTreeCollection for SimulatorModel {
    fn ref_tree_entities(&self) -> impl Iterator<Item = &dyn RefTreeEntity> {
        self.query_engine
            .ref_tree_entities()
            .chain(
                self.tasks
                    .values()
                    .map(|entity| entity as &dyn RefTreeEntity),
            )
            .chain(
                self.task_executors
                    .values()
                    .map(|entity| entity as &dyn RefTreeEntity),
            )
            .chain(
                self.networks
                    .values()
                    .map(|entity| entity as &dyn RefTreeEntity),
            )
            .chain(
                self.gpus
                    .values()
                    .map(|entity| entity as &dyn RefTreeEntity),
            )
            .chain(
                self.host_memories
                    .values()
                    .map(|entity| entity as &dyn RefTreeEntity),
            )
            .chain(
                self.storages
                    .values()
                    .map(|entity| entity as &dyn RefTreeEntity),
            )
            .chain(
                self.gpu_memories
                    .values()
                    .map(|entity| entity as &dyn RefTreeEntity),
            )
            .chain(
                self.task_executor_threads
                    .values()
                    .map(|entity| entity as &dyn RefTreeEntity),
            )
            .chain(
                self.storage_channels
                    .values()
                    .map(|entity| entity as &dyn RefTreeEntity),
            )
            .chain(
                self.pcie_channels
                    .values()
                    .map(|entity| entity as &dyn RefTreeEntity),
            )
            .chain(
                self.network_channels
                    .values()
                    .map(|entity| entity as &dyn RefTreeEntity),
            )
    }

    fn ref_tree_entity(&self, entity_id: Uuid) -> AnalyzerResult<&dyn RefTreeEntity> {
        if let Ok(entity) = self.query_engine.ref_tree_entity(entity_id) {
            Ok(entity)
        } else if let Some(entity) = self.tasks.get(&entity_id) {
            Ok(entity)
        } else if let Some(entity) = self.task_executors.get(&entity_id) {
            Ok(entity)
        } else if let Some(entity) = self.networks.get(&entity_id) {
            Ok(entity)
        } else if let Some(entity) = self.gpus.get(&entity_id) {
            Ok(entity)
        } else if let Some(entity) = self.host_memories.get(&entity_id) {
            Ok(entity)
        } else if let Some(entity) = self.storages.get(&entity_id) {
            Ok(entity)
        } else if let Some(entity) = self.gpu_memories.get(&entity_id) {
            Ok(entity)
        } else if let Some(entity) = self.task_executor_threads.get(&entity_id) {
            Ok(entity)
        } else if let Some(entity) = self.storage_channels.get(&entity_id) {
            Ok(entity)
        } else if let Some(entity) = self.pcie_channels.get(&entity_id) {
            Ok(entity)
        } else if let Some(entity) = self.network_channels.get(&entity_id) {
            Ok(entity)
        } else {
            Err(AnalyzerError::InvalidId(entity_id))
        }
    }
}

impl ResourceCollection for SimulatorModel {
    fn resources(&self) -> impl Iterator<Item = &dyn Resource> {
        self.simulator_resources()
            .chain(self.query_engine.resources())
    }
    fn resource_groups(&self) -> impl Iterator<Item = &dyn ResourceGroup> {
        self.task_executors
            .values()
            .map(|entity| entity as &dyn ResourceGroup)
            .chain(
                self.networks
                    .values()
                    .map(|entity| entity as &dyn ResourceGroup),
            )
            .chain(
                self.gpus
                    .values()
                    .map(|entity| entity as &dyn ResourceGroup),
            )
            .chain(self.query_engine.resource_groups())
    }
    fn resource(&self, resource_id: Uuid) -> AnalyzerResult<&dyn Resource> {
        self.simulator_resource(resource_id)
            .ok_or(AnalyzerError::InvalidId(resource_id))
            .or_else(|_| self.query_engine.resource(resource_id))
    }
    fn resource_type(&self, resource_type_name: &str) -> AnalyzerResult<&ResourceTypeDecl> {
        self.query_engine
            .resource_type(resource_type_name)
            .or_else(|_| {
                self.resource_types
                    .get(resource_type_name)
                    .ok_or_else(|| AnalyzerError::InvalidTypeName(resource_type_name.to_owned()))
            })
    }
    fn resource_group(&self, resource_group_id: Uuid) -> AnalyzerResult<&dyn ResourceGroup> {
        self.query_engine
            .resource_group(resource_group_id)
            .or_else(|_| {
                self.task_executors
                    .get(&resource_group_id)
                    .map(|entity| entity as &dyn ResourceGroup)
                    .or_else(|| {
                        self.networks
                            .get(&resource_group_id)
                            .map(|entity| entity as &dyn ResourceGroup)
                    })
                    .or_else(|| {
                        self.gpus
                            .get(&resource_group_id)
                            .map(|entity| entity as &dyn ResourceGroup)
                    })
                    .ok_or(AnalyzerError::InvalidId(resource_group_id))
            })
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
            .task_executors
            .values()
            .map(|entity| entity as &dyn ResourceGroup)
            .chain(
                self.networks
                    .values()
                    .map(|entity| entity as &dyn ResourceGroup),
            )
            .chain(
                self.gpus
                    .values()
                    .map(|entity| entity as &dyn ResourceGroup),
            )
            .filter_map(move |group| {
                (group.parent_group_id() == Some(resource_group_id)).then_some(group.id())
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

        let sim = self.simulator_resources().filter_map(move |resource| {
            (resource.parent_group_id() == resource_group_id).then_some(resource.id())
        });

        Ok(engine.into_iter().flatten().chain(sim))
    }
}

impl Using for SimulatorModel {
    fn usages(&self) -> impl Iterator<Item = impl Usage<'_>> {
        self.tasks.values().flat_map(|task| task.usages())
    }
}

pub struct SimulatorModelBuilder {
    query_engine: InMemoryQueryEngineModelBuilder,
    host_memories: HashMap<Uuid, HostMemory>,
    storages: HashMap<Uuid, Storage>,
    gpu_memories: HashMap<Uuid, GpuMemory>,
    task_executor_threads: HashMap<Uuid, TaskExecutorThread>,
    storage_channels: HashMap<Uuid, StorageChannel>,
    pcie_channels: HashMap<Uuid, PcieChannel>,
    network_channels: HashMap<Uuid, NetworkChannel>,
    task_executors: HashMap<Uuid, TaskExecutor>,
    networks: HashMap<Uuid, Network>,
    gpus: HashMap<Uuid, Gpu>,
    tasks: HashMap<Uuid, TaskBuilder>,
}

impl SimulatorModelBuilder {
    pub(crate) fn try_new(engine_id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self {
            query_engine: InMemoryQueryEngineModelBuilder::try_new(engine_id)?,
            host_memories: HashMap::default(),
            storages: HashMap::default(),
            gpu_memories: HashMap::default(),
            task_executor_threads: HashMap::default(),
            storage_channels: HashMap::default(),
            pcie_channels: HashMap::default(),
            network_channels: HashMap::default(),
            task_executors: HashMap::default(),
            networks: HashMap::default(),
            gpus: HashMap::default(),
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
                task_builder.push_transition(Event::new(id, timestamp, t));
                Ok(())
            }
            SimulatorEvent::Engine(event) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                event.into_query_engine_event(),
            )),
            SimulatorEvent::Worker(event) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                event.into_query_engine_event(),
            )),
            SimulatorEvent::QueryGroup(event) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                event.into_query_engine_event(),
            )),
            SimulatorEvent::Query(event) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                event.into_query_engine_event(),
            )),
            SimulatorEvent::Plan(event) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                event.into_query_engine_event(),
            )),
            SimulatorEvent::Operator(event) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                event.into_query_engine_event(),
            )),
            SimulatorEvent::Port(event) => self.query_engine.try_push(Event::new(
                id,
                timestamp,
                event.into_query_engine_event(),
            )),
            SimulatorEvent::HostMemory(event) => {
                let event = Event::new(id, timestamp, event);
                if let Some(resource) = self.host_memories.get_mut(&id) {
                    resource.push(event)
                } else {
                    self.host_memories
                        .insert(id, HostMemory::try_from_event(event)?);
                    Ok(())
                }
            }
            SimulatorEvent::StorageChannel(event) => {
                let event = Event::new(id, timestamp, event);
                if let Some(resource) = self.storage_channels.get_mut(&id) {
                    resource.push(event)
                } else {
                    self.storage_channels
                        .insert(id, StorageChannel::try_from_event(event)?);
                    Ok(())
                }
            }
            SimulatorEvent::Storage(event) => {
                let event = Event::new(id, timestamp, event);
                if let Some(resource) = self.storages.get_mut(&id) {
                    resource.push(event)
                } else {
                    self.storages.insert(id, Storage::try_from_event(event)?);
                    Ok(())
                }
            }
            SimulatorEvent::GpuMemory(event) => {
                let event = Event::new(id, timestamp, event);
                if let Some(resource) = self.gpu_memories.get_mut(&id) {
                    resource.push(event)
                } else {
                    self.gpu_memories
                        .insert(id, GpuMemory::try_from_event(event)?);
                    Ok(())
                }
            }
            SimulatorEvent::TaskExecutorThread(event) => {
                let event = Event::new(id, timestamp, event);
                if let Some(resource) = self.task_executor_threads.get_mut(&id) {
                    resource.push(event)
                } else {
                    self.task_executor_threads
                        .insert(id, TaskExecutorThread::try_from_event(event)?);
                    Ok(())
                }
            }
            SimulatorEvent::PcieChannel(event) => {
                let event = Event::new(id, timestamp, event);
                if let Some(resource) = self.pcie_channels.get_mut(&id) {
                    resource.push(event)
                } else {
                    self.pcie_channels
                        .insert(id, PcieChannel::try_from_event(event)?);
                    Ok(())
                }
            }
            SimulatorEvent::NetworkChannel(event) => {
                let event = Event::new(id, timestamp, event);
                if let Some(resource) = self.network_channels.get_mut(&id) {
                    resource.push(event)
                } else {
                    self.network_channels
                        .insert(id, NetworkChannel::try_from_event(event)?);
                    Ok(())
                }
            }
            SimulatorEvent::TaskExecutor(event) => {
                let event = Event::new(id, timestamp, event);
                if let Some(entity) = self.task_executors.get_mut(&id) {
                    entity.push(event)
                } else {
                    self.task_executors
                        .insert(id, TaskExecutor::try_from_event(event)?);
                    Ok(())
                }
            }
            SimulatorEvent::Network(event) => {
                let event = Event::new(id, timestamp, event);
                if let Some(entity) = self.networks.get_mut(&id) {
                    entity.push(event)
                } else {
                    self.networks.insert(id, Network::try_from_event(event)?);
                    Ok(())
                }
            }
            SimulatorEvent::Gpu(event) => {
                let event = Event::new(id, timestamp, event);
                if let Some(entity) = self.gpus.get_mut(&id) {
                    entity.push(event)
                } else {
                    self.gpus.insert(id, Gpu::try_from_event(event)?);
                    Ok(())
                }
            }
        }
    }

    pub(crate) fn try_build(self) -> AnalyzerResult<SimulatorModel> {
        let resource_types = [
            HostMemory::resource_type_decl(),
            Storage::resource_type_decl(),
            GpuMemory::resource_type_decl(),
            TaskExecutorThread::resource_type_decl(),
            StorageChannel::resource_type_decl(),
            PcieChannel::resource_type_decl(),
            NetworkChannel::resource_type_decl(),
        ]
        .into_iter()
        .map(|declaration| (declaration.name.clone(), declaration))
        .collect();

        let mut model = SimulatorModel {
            query_engine: self.query_engine.try_build()?,
            resource_types,
            host_memories: self.host_memories,
            storages: self.storages,
            gpu_memories: self.gpu_memories,
            task_executor_threads: self.task_executor_threads,
            storage_channels: self.storage_channels,
            pcie_channels: self.pcie_channels,
            network_channels: self.network_channels,
            task_executors: self.task_executors,
            networks: self.networks,
            gpus: self.gpus,
            tasks: HashMap::default(),
            resource_group_types: HashMap::default(),
        };

        for (task_id, task_builder) in self.tasks.into_iter() {
            let task = Task::from_builder(task_builder)?;
            for usage in task.usages() {
                let resource_type_name = model
                    .resource(usage.resource_id())
                    .map(Entity::type_name)?
                    .to_owned();
                let set = &mut model
                    .resource_types
                    .get_mut(&resource_type_name)
                    .ok_or_else(|| AnalyzerError::InvalidTypeName(resource_type_name.clone()))?
                    .used_by;
                if !set.contains(task.type_name()) {
                    set.insert(task.type_name().to_owned());
                }
            }
            if let Some(operator_id) = task.operator_id()
                && let Some(task_span) = task.active_span()
                && let Ok(operator) = model.query_engine.operator_mut(operator_id)
            {
                operator.extend_active_span(task_span);
            }

            model.tasks.insert(task_id, task);
        }

        RefTreeNode::try_new(&model)?;
        let mut resource_group_types = derive_resource_group_types(&model)?;
        // Bubble up all the used_by_entity fields in the group type decls.
        for group_type_decl in resource_group_types.values_mut() {
            for contained_resource_type in &group_type_decl.contains_resource_types {
                if let Ok(resource_type) = model.resource_type(contained_resource_type) {
                    for entity_type in &resource_type.used_by {
                        group_type_decl
                            .used_by_entity_types
                            .insert(entity_type.clone());
                    }
                }
            }
        }
        model.resource_group_types = resource_group_types;
        Ok(model)
    }
}
