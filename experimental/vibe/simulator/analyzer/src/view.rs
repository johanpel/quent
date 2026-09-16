// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_analyzer::{
    AnalyzerError, AnalyzerResult, Entity, Model,
    resource::{
        Resource, ResourceGroup, ResourceTypeDecl, Usage, Using, collection::ResourceCollection,
    },
};
use quent_query_engine_analyzer::{QueryEngineModel, plan_tree::PlanTree};
use quent_query_engine_ui::EntityRef;
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use uuid::Uuid;

use crate::{
    boilerplate::{
        Engine, Gpu, Network, Operator, Plan, Port, Query, QueryGroup, TaskExecutor, Worker,
    },
    model::SimulatorModel,
    task::Task,
};

/// A view of the simulator model filtered to a specific query
// TODO(johanpel): figure out a better way to construct these views, or to
// filter the data on a per query basis. This is generally tricky because the
// state of resources of engines that are shared across query groups or across
// the entire engine could be modified by other queries.
pub(crate) struct SimulatorModelQueryView<'a> {
    resource_types: HashMap<String, &'a ResourceTypeDecl>,
    engine: &'a Engine,
    query_group: &'a QueryGroup,
    query: &'a Query,
    workers: HashMap<Uuid, &'a Worker>,
    plans: HashMap<Uuid, &'a Plan>,
    operators: HashMap<Uuid, &'a Operator>,
    ports: HashMap<Uuid, &'a Port>,
    resources: HashMap<Uuid, &'a dyn Resource>,
    task_executors: HashMap<Uuid, &'a TaskExecutor>,
    networks: HashMap<Uuid, &'a Network>,
    gpus: HashMap<Uuid, &'a Gpu>,
    tasks: HashMap<Uuid, &'a Task>,
}

impl<'a> SimulatorModelQueryView<'a> {
    pub fn try_new(
        model: &'a SimulatorModel,
        query_id: Uuid,
    ) -> AnalyzerResult<SimulatorModelQueryView<'a>> {
        let query = model.query(query_id)?;
        let query_group = model.query_group(query.query_group_id().unwrap_or_default())?;
        let workers: HashMap<Uuid, &Worker> = model
            .query_workers(query_id)?
            .map(|entity| (entity.id(), entity))
            .collect();
        let plans: HashMap<Uuid, &Plan> = model
            .query_plans(query_id)?
            .map(|entity| (entity.id(), entity))
            .collect();
        let operators: HashMap<Uuid, &Operator> = model
            .plans_operators(plans.values().copied())?
            .map(|entity| (entity.id(), entity))
            .collect();
        let ports: HashMap<Uuid, &Port> = model
            .operators_ports(operators.values().copied())?
            .map(|entity| (entity.id(), entity))
            .collect();
        let query_engine_group_ids: HashSet<Uuid> = std::iter::once(model.engine.id())
            .chain(std::iter::once(query_group.id()))
            .chain(std::iter::once(query.id()))
            .chain(workers.keys().copied())
            .chain(plans.keys().copied())
            .chain(operators.keys().copied())
            .chain(ports.keys().copied())
            .collect();

        let task_executors = model
            .task_executors
            .iter()
            .filter(|(_, entity)| {
                entity
                    .parent_group_id()
                    .is_some_and(|parent| query_engine_group_ids.contains(&parent))
            })
            .map(|(id, entity)| (*id, entity))
            .collect::<HashMap<_, _>>();
        let networks = model
            .networks
            .iter()
            .filter(|(_, entity)| {
                entity
                    .parent_group_id()
                    .is_some_and(|parent| query_engine_group_ids.contains(&parent))
            })
            .map(|(id, entity)| (*id, entity))
            .collect::<HashMap<_, _>>();
        let gpus = model
            .gpus
            .iter()
            .filter(|(_, entity)| {
                entity
                    .parent_group_id()
                    .is_some_and(|parent| query_engine_group_ids.contains(&parent))
            })
            .map(|(id, entity)| (*id, entity))
            .collect::<HashMap<_, _>>();

        let resources = model
            .resources()
            .filter(|resource| {
                // This needs to reference a QE resource group:
                let in_qe = query_engine_group_ids.contains(&resource.parent_group_id());
                // Or a simulator entity that groups resources.
                let parent_id = resource.parent_group_id();
                let in_sim = task_executors.contains_key(&parent_id)
                    || networks.contains_key(&parent_id)
                    || gpus.contains_key(&parent_id);
                in_qe || in_sim
            })
            .map(|resource| (resource.id(), resource))
            .collect::<HashMap<_, _>>();

        let resource_types = model
            .resource_types
            .iter()
            .map(|(k, v)| (k.clone(), v))
            .collect();

        let mut result = SimulatorModelQueryView {
            resource_types,
            engine: &model.engine,
            query_group,
            query,
            workers,
            plans,
            operators,
            ports,
            resources,
            task_executors,
            networks,
            gpus,
            tasks: HashMap::default(),
        };

        result.tasks = model
            .tasks
            .values()
            .map(|task| (task.id(), task))
            .filter(|(_, task)| {
                task.usages()
                    .any(|usage| result.resource(usage.resource_id()).is_ok())
            })
            .collect();
        Ok(result)
    }

    fn query_engine_resource_groups(&self) -> impl Iterator<Item = &dyn ResourceGroup> {
        std::iter::once(self.engine as &dyn ResourceGroup)
            .chain(std::iter::once(self.query_group as &dyn ResourceGroup))
            .chain(std::iter::once(self.query as &dyn ResourceGroup))
            .chain(
                self.workers
                    .values()
                    .map(|entity| *entity as &dyn ResourceGroup),
            )
            .chain(
                self.plans
                    .values()
                    .map(|entity| *entity as &dyn ResourceGroup),
            )
            .chain(
                self.operators
                    .values()
                    .map(|entity| *entity as &dyn ResourceGroup),
            )
            .chain(
                self.ports
                    .values()
                    .map(|entity| *entity as &dyn ResourceGroup),
            )
    }

    fn query_engine_resource_group(&self, id: Uuid) -> Option<&dyn ResourceGroup> {
        (self.engine.id() == id)
            .then_some(self.engine as &dyn ResourceGroup)
            .or_else(|| {
                (self.query_group.id() == id).then_some(self.query_group as &dyn ResourceGroup)
            })
            .or_else(|| (self.query.id() == id).then_some(self.query as &dyn ResourceGroup))
            .or_else(|| {
                self.workers
                    .get(&id)
                    .map(|entity| *entity as &dyn ResourceGroup)
            })
            .or_else(|| {
                self.plans
                    .get(&id)
                    .map(|entity| *entity as &dyn ResourceGroup)
            })
            .or_else(|| {
                self.operators
                    .get(&id)
                    .map(|entity| *entity as &dyn ResourceGroup)
            })
            .or_else(|| {
                self.ports
                    .get(&id)
                    .map(|entity| *entity as &dyn ResourceGroup)
            })
    }
}

impl<'a> QueryEngineModel for SimulatorModelQueryView<'a> {
    type Engine = Engine;
    type Query = Query;
    type QueryGroup = QueryGroup;
    type Worker = Worker;
    type Plan = Plan;
    type Operator = Operator;
    type Port = Port;

    fn engine(&self) -> AnalyzerResult<&Engine> {
        Ok(self.engine)
    }
    fn query(&self, query_id: Uuid) -> AnalyzerResult<&Query> {
        (self.query.id() == query_id)
            .then_some(self.query)
            .ok_or(AnalyzerError::InvalidId(query_id))
    }
    fn query_group(&self, query_group_id: Uuid) -> AnalyzerResult<&QueryGroup> {
        (self.query_group.id() == query_group_id)
            .then_some(self.query_group)
            .ok_or(AnalyzerError::InvalidId(query_group_id))
    }
    fn worker(&self, worker_id: Uuid) -> AnalyzerResult<&Worker> {
        self.workers
            .get(&worker_id)
            .copied()
            .ok_or(AnalyzerError::InvalidId(worker_id))
    }
    fn plan(&self, plan_id: Uuid) -> AnalyzerResult<&Plan> {
        self.plans
            .get(&plan_id)
            .copied()
            .ok_or(AnalyzerError::InvalidId(plan_id))
    }
    fn operator(&self, operator_id: Uuid) -> AnalyzerResult<&Operator> {
        self.operators
            .get(&operator_id)
            .copied()
            .ok_or(AnalyzerError::InvalidId(operator_id))
    }
    fn port(&self, port_id: Uuid) -> AnalyzerResult<&Port> {
        self.ports
            .get(&port_id)
            .copied()
            .ok_or(AnalyzerError::InvalidId(port_id))
    }
    fn queries(&self) -> impl Iterator<Item = &Query> {
        std::iter::once(self.query)
    }
    fn query_groups(&self) -> impl Iterator<Item = &QueryGroup> {
        std::iter::once(self.query_group)
    }
    fn workers(&self) -> impl Iterator<Item = &Worker> {
        self.workers.values().copied()
    }
    fn plans(&self) -> impl Iterator<Item = &Plan> {
        self.plans.values().copied()
    }
    fn operators(&self) -> impl Iterator<Item = &Operator> {
        self.operators.values().copied()
    }
    fn ports(&self) -> impl Iterator<Item = &Port> {
        self.ports.values().copied()
    }
    fn plan_tree(&self, query_id: Uuid) -> AnalyzerResult<PlanTree> {
        PlanTree::try_new(self.plans.values().copied(), query_id)
    }
}

impl<'a> Model for SimulatorModelQueryView<'a> {
    type EntityIdType = EntityRef;
    fn try_entity_ref(&self, entity_id: Uuid) -> AnalyzerResult<Self::EntityIdType> {
        if self.engine.id() == entity_id {
            Ok(EntityRef::Engine(entity_id))
        } else if self.workers.contains_key(&entity_id) {
            Ok(EntityRef::Worker(entity_id))
        } else if self.query_group.id() == entity_id {
            Ok(EntityRef::QueryGroup(entity_id))
        } else if self.query.id() == entity_id {
            Ok(EntityRef::Query(entity_id))
        } else if self.plans.contains_key(&entity_id) {
            Ok(EntityRef::Plan(entity_id))
        } else if self.operators.contains_key(&entity_id) {
            Ok(EntityRef::Operator(entity_id))
        } else if self.ports.contains_key(&entity_id) {
            Ok(EntityRef::Port(entity_id))
        } else if self.resources.contains_key(&entity_id) {
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
        Ok(self.engine)
    }
}

impl<'a> ResourceCollection for SimulatorModelQueryView<'a> {
    fn resources(&self) -> impl Iterator<Item = &dyn Resource> {
        self.resources.values().map(|r| *r as &dyn Resource)
    }
    fn resource_groups(&self) -> impl Iterator<Item = &dyn ResourceGroup> {
        let task_executors = self
            .task_executors
            .values()
            .map(|r| *r as &dyn ResourceGroup);
        let networks = self.networks.values().map(|r| *r as &dyn ResourceGroup);
        let gpus = self.gpus.values().map(|r| *r as &dyn ResourceGroup);
        self.query_engine_resource_groups()
            .chain(task_executors)
            .chain(networks)
            .chain(gpus)
    }
    fn resource(&self, resource_id: Uuid) -> AnalyzerResult<&dyn Resource> {
        // qe model has no leaf resources.
        self.resources
            .get(&resource_id)
            .map(|r| *r as &dyn Resource)
            .ok_or(AnalyzerError::InvalidId(resource_id))
    }
    fn resource_type(&self, resource_type_name: &str) -> AnalyzerResult<&ResourceTypeDecl> {
        self.resource_types
            .get(resource_type_name)
            .copied()
            .ok_or_else(|| AnalyzerError::InvalidTypeName(resource_type_name.to_owned()))
    }
    fn resource_group(&self, resource_group_id: Uuid) -> AnalyzerResult<&dyn ResourceGroup> {
        self.query_engine_resource_group(resource_group_id)
            .or_else(|| {
                self.task_executors
                    .get(&resource_group_id)
                    .map(|r| *r as &dyn ResourceGroup)
            })
            .or_else(|| {
                self.networks
                    .get(&resource_group_id)
                    .map(|r| *r as &dyn ResourceGroup)
            })
            .or_else(|| {
                self.gpus
                    .get(&resource_group_id)
                    .map(|r| *r as &dyn ResourceGroup)
            })
            .ok_or(AnalyzerError::InvalidId(resource_group_id))
    }
    fn resource_group_child_groups(
        &self,
        resource_group_id: Uuid,
    ) -> AnalyzerResult<impl Iterator<Item = Uuid>> {
        self.resource_group(resource_group_id)?;
        Ok(self.resource_groups().filter_map(move |g| {
            g.parent_group_id()
                .is_some_and(|p| p == resource_group_id)
                .then_some(g.id())
        }))
    }
    fn resource_group_child_resources(
        &self,
        resource_group_id: Uuid,
    ) -> AnalyzerResult<impl Iterator<Item = Uuid>> {
        // Verify the resource group exists
        self.resource_group(resource_group_id)?;
        Ok(self
            .resources()
            .filter_map(move |r| (r.parent_group_id() == resource_group_id).then_some(r.id())))
    }
}
