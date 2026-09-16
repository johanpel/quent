// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Reusable in-memory query-engine model and semantic ingestion events.

use quent_analyzer::{
    AnalyzerError, AnalyzerResult, Entity, Model, ScopedEntity, Span,
    entity::{
        collection::ScopeCollection,
        native::{AnalyzedEntity, EntityEventAccumulator},
    },
    fsm::{
        Fsm, FsmUsages,
        native::{
            AnalyzedFsm as NativeFsm, DynamicAttribute, FsmBuilder, Transition as NativeTransition,
        },
    },
    resource::{
        Resource, ResourceGroup, ResourceTypeDecl, Usage, Using, collection::ResourceCollection,
    },
};
use quent_events::{DynamicAttributes, EntityEvent, Event};
use quent_query_engine_ui as ui;
use quent_time::{TimeUnixNanoSec, Timestamp, span::SpanUnixNanoSec, try_to_secs_relative};
use rustc_hash::FxHashMap as HashMap;
use uuid::Uuid;

use crate::{
    EngineEntity, OperatorEntity, OperatorEntityMut, PlanEntity, PortEntity, QueryEngineEntityId,
    QueryEngineModel, QueryEngineModelMut, QueryEntity, QueryGroupEntity, WorkerEntity,
    plan_tree::PlanTree,
};

/// Events accepted by [`InMemoryQueryEngineModelBuilder`].
///
/// Applications convert their schema-generated events into this semantic event
/// family before ingestion. Conversion consumes owned values and therefore does
/// not require cloning strings or dynamic attributes.
// TODO(johanpel): Model query-engine semantics, including DAG constraints, and
// generate this event family and its schema adapters from that model. See
// https://github.com/rapidsai/quent/issues/288.
#[derive(Debug)]
pub enum QueryEngineEvent {
    Engine(EngineEvent),
    Worker(WorkerEvent),
    QueryGroup(QueryGroupEvent),
    Query(QueryEvent),
    Plan(PlanEvent),
    Operator(OperatorEvent),
    Port(PortEvent),
}

/// Query-engine implementation metadata carried by [`EngineEvent::Init`].
#[derive(Debug)]
pub struct EngineImplementation {
    pub name: Option<String>,
    pub version: Option<String>,
    pub custom_attributes: DynamicAttributes,
}

/// Lifecycle events for an engine.
#[derive(Debug)]
pub enum EngineEvent {
    Init {
        implementation: EngineImplementation,
        instance_name: Option<String>,
    },
    Exit,
}

impl EntityEvent for EngineEvent {
    const NAME: &'static str = "Engine";
}

/// Lifecycle events for a worker.
#[derive(Debug)]
pub enum WorkerEvent {
    Init {
        parent_engine_id: Uuid,
        instance_name: String,
    },
    Exit,
}

impl EntityEvent for WorkerEvent {
    const NAME: &'static str = "Worker";
}

/// Declaration data for a query group.
#[derive(Debug)]
pub struct QueryGroupEvent {
    pub instance_name: String,
    pub engine_id: Uuid,
}

impl EntityEvent for QueryGroupEvent {
    const NAME: &'static str = "QueryGroup";
}

/// State transitions for a query.
#[derive(Debug)]
pub enum QueryEvent {
    Init {
        seq: u16,
        instance_name: String,
        query_group_id: Uuid,
    },
    Planning {
        seq: u16,
    },
    Executing {
        seq: u16,
    },
    Done {
        seq: u16,
    },
}

impl EntityEvent for QueryEvent {
    const NAME: &'static str = "Query";
}

impl quent_analyzer::fsm::native::TransitionEvent for QueryEvent {
    fn entity_type_name() -> &'static str {
        "query"
    }

    fn sequence(&self) -> u16 {
        match self {
            Self::Init { seq, .. }
            | Self::Planning { seq }
            | Self::Executing { seq }
            | Self::Done { seq } => *seq,
        }
    }

    fn is_final(&self) -> bool {
        matches!(self, Self::Done { .. })
    }

    fn state_name(&self) -> &'static str {
        match self {
            Self::Init { .. } => "init",
            Self::Planning { .. } => "planning",
            Self::Executing { .. } => "executing",
            Self::Done { .. } => "done",
        }
    }

    fn dynamic_attributes(&self) -> Vec<DynamicAttribute> {
        match self {
            Self::Init { instance_name, .. } => vec![DynamicAttribute::string(
                "instance_name",
                instance_name.clone(),
            )],
            _ => Vec::new(),
        }
    }
}

/// A directed connection between two ports.
#[derive(Debug)]
pub struct Edge {
    pub source: Uuid,
    pub target: Uuid,
}

/// The query and optional predecessor plan associated with a plan.
#[derive(Debug)]
pub struct PlanParent {
    pub query_id: Uuid,
    pub plan_id: Option<Uuid>,
}

/// Declaration data for a plan.
#[derive(Debug)]
pub struct PlanEvent {
    pub parent: PlanParent,
    pub instance_name: String,
    pub edges: Vec<Edge>,
    pub worker_id: Option<Uuid>,
}

impl EntityEvent for PlanEvent {
    const NAME: &'static str = "Plan";
}

/// Declaration and statistics events for an operator.
#[derive(Debug)]
pub enum OperatorEvent {
    Declaration {
        plan_id: Uuid,
        parent_operator_ids: Vec<Uuid>,
        instance_name: String,
        type_name: String,
        custom_attributes: DynamicAttributes,
    },
    Statistics {
        custom_attributes: DynamicAttributes,
    },
}

impl EntityEvent for OperatorEvent {
    const NAME: &'static str = "Operator";
}

/// Declaration and statistics events for a port.
#[derive(Debug)]
pub enum PortEvent {
    Declaration {
        operator_id: Uuid,
        instance_name: String,
    },
    Statistics {
        custom_attributes: DynamicAttributes,
    },
}

impl EntityEvent for PortEvent {
    const NAME: &'static str = "Port";
}

fn instance_name_attribute(instance_name: Option<&str>) -> Vec<DynamicAttribute> {
    instance_name
        .map(|name| vec![DynamicAttribute::string("instance_name", name)])
        .unwrap_or_default()
}

#[derive(Default)]
struct EngineAccumulator {
    instance_name: Option<String>,
    implementation: Option<ui::EngineImplementationAttributes>,
}

impl quent_events::Entity for EngineAccumulator {
    type Event = EngineEvent;
}

impl EntityEventAccumulator for EngineAccumulator {
    fn push(&mut self, event: Self::Event) {
        if let EngineEvent::Init {
            implementation,
            instance_name,
        } = event
        {
            self.instance_name = instance_name;
            self.implementation = Some(ui::EngineImplementationAttributes {
                name: implementation.name,
                version: implementation.version,
                custom_attributes: implementation.custom_attributes.0,
            });
        }
    }
}

/// An engine accumulated from normalized query-engine events.
#[derive(Debug)]
pub struct Engine(Entity<EngineAccumulator>);

impl Engine {
    fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self(Entity::new(id)?))
    }

    fn push(&mut self, event: Event<EngineEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }
}

impl Entity for Engine {
    fn id(&self) -> Uuid {
        self.0.id()
    }
    fn type_name(&self) -> &str {
        "engine"
    }
    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.0
            .earliest_timestamp()
            .expect("analyzed engine must contain at least one event")
    }
    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.0
            .latest_timestamp()
            .expect("analyzed engine must contain at least one event")
    }
    fn attributes(&self) -> Vec<DynamicAttribute> {
        instance_name_attribute(self.0.accumulator().instance_name.as_deref())
    }
}

impl Span for Engine {
    fn span(&self) -> AnalyzerResult<SpanUnixNanoSec> {
        match (self.0.earliest_timestamp(), self.0.latest_timestamp()) {
            (Some(start), Some(end)) => Ok(SpanUnixNanoSec::try_new(start, end)?),
            _ => Err(AnalyzerError::IncompleteEntity(
                "engine does not have both init and exit events".to_owned(),
            )),
        }
    }
}

impl ResourceGroup for Engine {
    fn parent_group_id(&self) -> Option<Uuid> {
        None
    }
}

impl ScopedEntity for Engine {
    fn scope_id(&self) -> Option<Uuid> {
        None
    }
}

impl EngineEntity for Engine {
    fn to_ui(&self) -> AnalyzerResult<ui::Engine> {
        let start = self.0.earliest_timestamp();
        let end = self.0.latest_timestamp();
        let duration_s = match (start, end) {
            (Some(start), Some(end)) => Some(try_to_secs_relative(end, start)?),
            _ => None,
        };
        Ok(ui::Engine {
            id: self.0.id(),
            start_time_unix_ns: start,
            duration_s,
            instance_name: self.0.accumulator().instance_name.clone(),
            implementation: self
                .0
                .accumulator()
                .implementation
                .as_ref()
                .map(|implementation| ui::EngineImplementationAttributes {
                    name: implementation.name.clone(),
                    version: implementation.version.clone(),
                    custom_attributes: implementation.custom_attributes.clone(),
                }),
        })
    }
}

#[derive(Default)]
struct WorkerAccumulator {
    parent_engine_id: Option<Uuid>,
    instance_name: Option<String>,
}

impl quent_events::Entity for WorkerAccumulator {
    type Event = WorkerEvent;
}

impl EntityEventAccumulator for WorkerAccumulator {
    fn push(&mut self, event: Self::Event) {
        if let WorkerEvent::Init {
            parent_engine_id,
            instance_name,
        } = event
        {
            self.parent_engine_id = Some(parent_engine_id);
            self.instance_name = Some(instance_name);
        }
    }
}

/// A worker accumulated from normalized query-engine events.
#[derive(Debug)]
pub struct Worker(Entity<WorkerAccumulator>);

impl Worker {
    fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self(Entity::new(id)?))
    }

    fn push(&mut self, event: Event<WorkerEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }
}

impl Entity for Worker {
    fn id(&self) -> Uuid {
        self.0.id()
    }
    fn type_name(&self) -> &str {
        "worker"
    }
    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.0
            .earliest_timestamp()
            .expect("analyzed worker must contain at least one event")
    }
    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.0
            .latest_timestamp()
            .expect("analyzed worker must contain at least one event")
    }
    fn attributes(&self) -> Vec<DynamicAttribute> {
        instance_name_attribute(self.0.accumulator().instance_name.as_deref())
    }
}

impl Span for Worker {
    fn span(&self) -> AnalyzerResult<SpanUnixNanoSec> {
        match (self.0.earliest_timestamp(), self.0.latest_timestamp()) {
            (Some(start), Some(end)) => Ok(SpanUnixNanoSec::try_new(start, end)?),
            _ => Err(AnalyzerError::IncompleteEntity(
                "worker does not have both init and exit events".to_owned(),
            )),
        }
    }
}

impl ResourceGroup for Worker {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.0.accumulator().parent_engine_id
    }
}

impl ScopedEntity for Worker {
    fn scope_id(&self) -> Option<Uuid> {
        self.0.accumulator().parent_engine_id
    }
}

impl WorkerEntity for Worker {
    fn to_ui(&self, _epoch: TimeUnixNanoSec) -> ui::Worker {
        ui::Worker {
            id: self.0.id(),
            parent_engine_id: self.0.accumulator().parent_engine_id,
            instance_name: self.0.accumulator().instance_name.clone(),
            start_unix_ns: self.0.earliest_timestamp(),
            end_unix_ns: self.0.latest_timestamp(),
        }
    }
}

#[derive(Default)]
struct QueryGroupAccumulator {
    engine_id: Option<Uuid>,
    instance_name: Option<String>,
}

impl quent_events::Entity for QueryGroupAccumulator {
    type Event = QueryGroupEvent;
}

impl EntityEventAccumulator for QueryGroupAccumulator {
    fn push(&mut self, event: Self::Event) {
        let QueryGroupEvent {
            instance_name,
            engine_id,
        } = event;
        self.instance_name = Some(instance_name);
        self.engine_id = Some(engine_id);
    }
}

/// A query group accumulated from normalized query-engine events.
#[derive(Debug)]
pub struct QueryGroup(Entity<QueryGroupAccumulator>);

impl QueryGroup {
    fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self(Entity::new(id)?))
    }

    fn push(&mut self, event: Event<QueryGroupEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }
}

impl Entity for QueryGroup {
    fn id(&self) -> Uuid {
        self.0.id()
    }
    fn type_name(&self) -> &str {
        "query group"
    }
    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.0
            .earliest_timestamp()
            .expect("analyzed query group must contain at least one event")
    }
    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.0
            .latest_timestamp()
            .expect("analyzed query group must contain at least one event")
    }
    fn attributes(&self) -> Vec<DynamicAttribute> {
        instance_name_attribute(self.0.accumulator().instance_name.as_deref())
    }
}

impl ResourceGroup for QueryGroup {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.0.accumulator().engine_id
    }
}

impl ScopedEntity for QueryGroup {
    fn scope_id(&self) -> Option<Uuid> {
        self.0.accumulator().engine_id
    }
}

impl QueryGroupEntity for QueryGroup {
    fn to_ui(&self) -> ui::QueryGroup {
        ui::QueryGroup {
            id: self.0.id(),
            instance_name: self.0.accumulator().instance_name.clone(),
            engine_id: self.0.accumulator().engine_id,
        }
    }
}

pub type QueryBuilder = FsmBuilder<QueryEvent>;

/// A query reconstructed from normalized state transitions.
#[derive(Debug)]
pub struct Query(NativeFsm<QueryEvent>);

impl Query {
    fn from_builder(builder: QueryBuilder) -> AnalyzerResult<Self> {
        Ok(Self(builder.try_build()?))
    }
}

impl QueryEntity for Query {
    fn query_group_id(&self) -> Option<Uuid> {
        match self.0.first_data()? {
            QueryEvent::Init { query_group_id, .. } => Some(*query_group_id),
            _ => None,
        }
    }

    fn to_ui(&self) -> AnalyzerResult<ui::Query> {
        let transitions = self.0.transitions();
        let epoch = transitions.first().map(Timestamp::timestamp);
        let mut planning_s = None;
        let mut executing_s = None;
        let mut completed_s = None;

        if let Some(epoch) = epoch {
            for (index, transition) in transitions.iter().enumerate() {
                match transition.data {
                    QueryEvent::Planning { .. } => {
                        planning_s = Some(try_to_secs_relative(transition.timestamp(), epoch)?);
                    }
                    QueryEvent::Executing { .. } => {
                        executing_s = Some(try_to_secs_relative(transition.timestamp(), epoch)?);
                        if let Some(next) = transitions.get(index + 1) {
                            completed_s = Some(try_to_secs_relative(next.timestamp(), epoch)?);
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(ui::Query {
            id: self.id(),
            query_group_id: self.query_group_id().unwrap_or_default(),
            instance_name: self.0.first_data().and_then(|event| match event {
                QueryEvent::Init { instance_name, .. } => Some(instance_name.clone()),
                _ => None,
            }),
            start_unix_ns: epoch,
            planning_s,
            executing_s,
            completed_s,
        })
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
    fn attributes(&self) -> Vec<DynamicAttribute> {
        self.0.attributes()
    }
}

impl Fsm for Query {
    type TransitionType = NativeTransition<QueryEvent>;
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

impl ResourceGroup for Query {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.query_group_id()
    }
}

impl ScopedEntity for Query {
    fn scope_id(&self) -> Option<Uuid> {
        self.query_group_id()
    }
}

#[derive(Default)]
struct PlanAccumulator {
    instance_name: Option<String>,
    parent_query_id: Option<Uuid>,
    parent_plan_id: Option<Uuid>,
    worker_id: Option<Uuid>,
    edges: Vec<(Uuid, Uuid)>,
}

impl quent_events::Entity for PlanAccumulator {
    type Event = PlanEvent;
}

impl EntityEventAccumulator for PlanAccumulator {
    fn push(&mut self, event: Self::Event) {
        let PlanEvent {
            parent,
            instance_name,
            edges,
            worker_id,
        } = event;
        self.instance_name = Some(instance_name);
        self.parent_query_id = Some(parent.query_id);
        self.parent_plan_id = parent.plan_id;
        self.worker_id = worker_id;
        self.edges = edges
            .into_iter()
            .map(|edge| (edge.source, edge.target))
            .collect();
    }
}

/// A plan accumulated from normalized query-engine events.
#[derive(Debug)]
pub struct Plan(Entity<PlanAccumulator>);

impl Plan {
    fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self(Entity::new(id)?))
    }

    fn push(&mut self, event: Event<PlanEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }
}

impl Entity for Plan {
    fn id(&self) -> Uuid {
        self.0.id()
    }
    fn type_name(&self) -> &str {
        "plan"
    }
    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.0
            .earliest_timestamp()
            .expect("analyzed plan must contain at least one event")
    }
    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.0
            .latest_timestamp()
            .expect("analyzed plan must contain at least one event")
    }
    fn attributes(&self) -> Vec<DynamicAttribute> {
        instance_name_attribute(self.0.accumulator().instance_name.as_deref())
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

impl ScopedEntity for Plan {
    fn scope_id(&self) -> Option<Uuid> {
        self.0.accumulator().parent_query_id
    }
}

impl PlanEntity for Plan {
    fn parent_query_id(&self) -> Option<Uuid> {
        let data = self.0.accumulator();
        data.parent_plan_id
            .is_none()
            .then_some(data.parent_query_id)
            .flatten()
    }
    fn parent_plan_id(&self) -> Option<Uuid> {
        self.0.accumulator().parent_plan_id
    }
    fn worker_id(&self) -> Option<Uuid> {
        self.0.accumulator().worker_id
    }
    fn edges(&self) -> impl Iterator<Item = (Uuid, Uuid)> + '_ {
        self.0.accumulator().edges.iter().copied()
    }
    fn to_ui(&self) -> ui::Plan {
        ui::Plan {
            id: self.0.id(),
            instance_name: self.0.accumulator().instance_name.clone(),
            parent: self
                .0
                .accumulator()
                .parent_plan_id
                .or(self.0.accumulator().parent_query_id),
            worker_id: self.0.accumulator().worker_id,
            edges: self
                .0
                .accumulator()
                .edges
                .iter()
                .map(|&(source, target)| ui::Edge { source, target })
                .collect(),
        }
    }
}

#[derive(Default)]
struct OperatorAccumulator {
    plan_id: Option<Uuid>,
    parent_operator_ids: Vec<Uuid>,
    instance_name: Option<String>,
    operator_type_name: Option<String>,
    custom_attributes: DynamicAttributes,
    statistics: Option<DynamicAttributes>,
}

impl quent_events::Entity for OperatorAccumulator {
    type Event = OperatorEvent;
}

impl EntityEventAccumulator for OperatorAccumulator {
    fn push(&mut self, event: Self::Event) {
        match event {
            OperatorEvent::Declaration {
                plan_id,
                parent_operator_ids,
                instance_name,
                type_name,
                custom_attributes,
            } => {
                self.plan_id = Some(plan_id);
                self.parent_operator_ids = parent_operator_ids;
                self.instance_name = Some(instance_name);
                self.operator_type_name = Some(type_name);
                self.custom_attributes = custom_attributes;
            }
            OperatorEvent::Statistics { custom_attributes } => {
                self.statistics = Some(custom_attributes);
            }
        }
    }
}

/// An operator accumulated from normalized query-engine events.
#[derive(Debug)]
pub struct Operator {
    inner: Entity<OperatorAccumulator>,
    active_span: Option<SpanUnixNanoSec>,
}

impl Operator {
    fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self {
            inner: Entity::new(id)?,
            active_span: None,
        })
    }

    fn push(&mut self, event: Event<OperatorEvent>) -> AnalyzerResult<()> {
        self.inner.push(event)
    }
}

impl Entity for Operator {
    fn id(&self) -> Uuid {
        self.inner.id()
    }
    fn type_name(&self) -> &str {
        "operator"
    }
    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.inner
            .earliest_timestamp()
            .expect("analyzed operator must contain at least one event")
    }
    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.inner
            .latest_timestamp()
            .expect("analyzed operator must contain at least one event")
    }
    fn attributes(&self) -> Vec<DynamicAttribute> {
        instance_name_attribute(self.inner.accumulator().instance_name.as_deref())
    }
}

impl ResourceGroup for Operator {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.inner.accumulator().plan_id
    }
}

impl ScopedEntity for Operator {
    fn scope_id(&self) -> Option<Uuid> {
        self.inner.accumulator().plan_id
    }
}

impl OperatorEntity for Operator {
    fn plan_id(&self) -> Option<Uuid> {
        self.inner.accumulator().plan_id
    }
    fn parent_operator_ids(&self) -> impl ExactSizeIterator<Item = Uuid> + '_ {
        self.inner.accumulator().parent_operator_ids.iter().copied()
    }
    fn active_span(&self) -> Option<SpanUnixNanoSec> {
        self.active_span
    }
    fn operator_type_name(&self) -> Option<&str> {
        self.inner.accumulator().operator_type_name.as_deref()
    }
    fn to_ui(&self, epoch: TimeUnixNanoSec) -> ui::Operator {
        ui::Operator {
            id: self.inner.id(),
            plan_id: self.inner.accumulator().plan_id,
            parent_operator_ids: self.inner.accumulator().parent_operator_ids.clone(),
            instance_name: self.inner.accumulator().instance_name.clone(),
            operator_type_name: self.inner.accumulator().operator_type_name.clone(),
            custom_attributes: self
                .inner
                .accumulator()
                .custom_attributes
                .iter()
                .map(|attribute| (attribute.key.clone(), attribute.value.clone()))
                .collect(),
            statistics: self
                .inner
                .accumulator()
                .statistics
                .as_ref()
                .map(|statistics| ui::OperatorStatistics {
                    custom_statistics: statistics
                        .iter()
                        .map(|attribute| {
                            (
                                attribute.key.clone(),
                                ui::OperatorStatistic {
                                    value: attribute.value.clone(),
                                    quantity: None,
                                },
                            )
                        })
                        .collect(),
                }),
            active_span: self
                .active_span
                .and_then(|span| span.try_to_secs_relative(epoch).ok()),
        }
    }
}

impl OperatorEntityMut for Operator {
    fn extend_active_span(&mut self, span: SpanUnixNanoSec) {
        self.active_span = Some(match self.active_span {
            Some(existing) => existing.extend(&span),
            None => span,
        });
    }
}

#[derive(Default)]
struct PortAccumulator {
    operator_id: Option<Uuid>,
    instance_name: Option<String>,
    statistics: Option<DynamicAttributes>,
}

impl quent_events::Entity for PortAccumulator {
    type Event = PortEvent;
}

impl EntityEventAccumulator for PortAccumulator {
    fn push(&mut self, event: Self::Event) {
        match event {
            PortEvent::Declaration {
                operator_id,
                instance_name,
            } => {
                self.operator_id = Some(operator_id);
                self.instance_name = Some(instance_name);
            }
            PortEvent::Statistics { custom_attributes } => {
                self.statistics = Some(custom_attributes);
            }
        }
    }
}

/// A port accumulated from normalized query-engine events.
#[derive(Debug)]
pub struct Port(Entity<PortAccumulator>);

impl Port {
    fn try_new(id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self(Entity::new(id)?))
    }

    fn push(&mut self, event: Event<PortEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }
}

impl Entity for Port {
    fn id(&self) -> Uuid {
        self.0.id()
    }
    fn type_name(&self) -> &str {
        "port"
    }
    fn earliest_timestamp(&self) -> TimeUnixNanoSec {
        self.0
            .earliest_timestamp()
            .expect("analyzed port must contain at least one event")
    }
    fn latest_timestamp(&self) -> TimeUnixNanoSec {
        self.0
            .latest_timestamp()
            .expect("analyzed port must contain at least one event")
    }
    fn attributes(&self) -> Vec<DynamicAttribute> {
        instance_name_attribute(self.0.accumulator().instance_name.as_deref())
    }
}

impl ResourceGroup for Port {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.0.accumulator().operator_id
    }
}

impl ScopedEntity for Port {
    fn scope_id(&self) -> Option<Uuid> {
        self.0.accumulator().operator_id
    }
}

impl PortEntity for Port {
    fn operator_id(&self) -> Option<Uuid> {
        self.0.accumulator().operator_id
    }
    fn to_ui(&self, _epoch: TimeUnixNanoSec) -> ui::Port {
        ui::Port {
            id: self.0.id(),
            operator_id: self.0.accumulator().operator_id,
            instance_name: self.0.accumulator().instance_name.clone(),
            statistics: self.0.accumulator().statistics.as_ref().map(|statistics| {
                ui::PortStatistics {
                    custom_statistics: statistics
                        .iter()
                        .map(|attribute| (attribute.key.clone(), attribute.value.clone()))
                        .collect(),
                }
            }),
        }
    }
}

#[derive(Debug)]
/// Concrete in-memory implementation of [`QueryEngineModel`].
pub struct InMemoryQueryEngineModel {
    pub engine: Engine,
    pub workers: HashMap<Uuid, Worker>,
    pub query_groups: HashMap<Uuid, QueryGroup>,
    pub queries: HashMap<Uuid, Query>,
    pub plans: HashMap<Uuid, Plan>,
    pub operators: HashMap<Uuid, Operator>,
    pub ports: HashMap<Uuid, Port>,
}

impl Model for InMemoryQueryEngineModel {
    type EntityIdType = QueryEngineEntityId;

    fn try_entity_ref(&self, id: Uuid) -> AnalyzerResult<Self::EntityIdType> {
        if self.engine.id() == id {
            Ok(QueryEngineEntityId::Engine(id))
        } else if self.workers.contains_key(&id) {
            Ok(QueryEngineEntityId::Worker(id))
        } else if self.query_groups.contains_key(&id) {
            Ok(QueryEngineEntityId::QueryGroup(id))
        } else if self.queries.contains_key(&id) {
            Ok(QueryEngineEntityId::Query(id))
        } else if self.plans.contains_key(&id) {
            Ok(QueryEngineEntityId::Plan(id))
        } else if self.operators.contains_key(&id) {
            Ok(QueryEngineEntityId::Operator(id))
        } else if self.ports.contains_key(&id) {
            Ok(QueryEngineEntityId::Port(id))
        } else {
            Err(AnalyzerError::InvalidId(id))
        }
    }

    fn root(&self) -> AnalyzerResult<&impl ResourceGroup> {
        Ok(&self.engine)
    }
}

impl QueryEngineModel for InMemoryQueryEngineModel {
    type Engine = Engine;
    type Query = Query;
    type QueryGroup = QueryGroup;
    type Worker = Worker;
    type Plan = Plan;
    type Operator = Operator;
    type Port = Port;

    fn engine(&self) -> AnalyzerResult<&Engine> {
        Ok(&self.engine)
    }
    fn query(&self, id: Uuid) -> AnalyzerResult<&Query> {
        self.queries.get(&id).ok_or(AnalyzerError::InvalidId(id))
    }
    fn query_group(&self, id: Uuid) -> AnalyzerResult<&QueryGroup> {
        self.query_groups
            .get(&id)
            .ok_or(AnalyzerError::InvalidId(id))
    }
    fn worker(&self, id: Uuid) -> AnalyzerResult<&Worker> {
        self.workers.get(&id).ok_or(AnalyzerError::InvalidId(id))
    }
    fn plan(&self, id: Uuid) -> AnalyzerResult<&Plan> {
        self.plans.get(&id).ok_or(AnalyzerError::InvalidId(id))
    }
    fn operator(&self, id: Uuid) -> AnalyzerResult<&Operator> {
        self.operators.get(&id).ok_or(AnalyzerError::InvalidId(id))
    }
    fn port(&self, id: Uuid) -> AnalyzerResult<&Port> {
        self.ports.get(&id).ok_or(AnalyzerError::InvalidId(id))
    }
    fn queries(&self) -> impl Iterator<Item = &Query> {
        self.queries.values()
    }
    fn query_groups(&self) -> impl Iterator<Item = &QueryGroup> {
        self.query_groups.values()
    }
    fn workers(&self) -> impl Iterator<Item = &Worker> {
        self.workers.values()
    }
    fn plans(&self) -> impl Iterator<Item = &Plan> {
        self.plans.values()
    }
    fn operators(&self) -> impl Iterator<Item = &Operator> {
        self.operators.values()
    }
    fn ports(&self) -> impl Iterator<Item = &Port> {
        self.ports.values()
    }
    fn plan_tree(&self, query_id: Uuid) -> AnalyzerResult<PlanTree> {
        PlanTree::try_new(self.plans.values(), query_id)
    }
}

impl QueryEngineModelMut for InMemoryQueryEngineModel {
    fn operator_mut(&mut self, id: Uuid) -> AnalyzerResult<&mut Operator> {
        self.operators
            .get_mut(&id)
            .ok_or(AnalyzerError::InvalidId(id))
    }
}

impl InMemoryQueryEngineModel {
    /// Returns a query-scoped view of this model.
    pub fn query_view(&self, query_id: Uuid) -> AnalyzerResult<InMemoryQueryEngineModelView<'_>> {
        InMemoryQueryEngineModelView::try_new(self, query_id)
    }
}

impl ScopeCollection for InMemoryQueryEngineModel {
    fn scoped_entities(&self) -> impl Iterator<Item = &dyn ScopedEntity> {
        std::iter::once(&self.engine as &dyn ScopedEntity)
            .chain(
                self.workers
                    .values()
                    .map(|entity| entity as &dyn ScopedEntity),
            )
            .chain(
                self.query_groups
                    .values()
                    .map(|entity| entity as &dyn ScopedEntity),
            )
            .chain(
                self.queries
                    .values()
                    .map(|entity| entity as &dyn ScopedEntity),
            )
            .chain(
                self.plans
                    .values()
                    .map(|entity| entity as &dyn ScopedEntity),
            )
            .chain(
                self.operators
                    .values()
                    .map(|entity| entity as &dyn ScopedEntity),
            )
            .chain(
                self.ports
                    .values()
                    .map(|entity| entity as &dyn ScopedEntity),
            )
    }

    fn scoped_entity(&self, entity_id: Uuid) -> AnalyzerResult<&dyn ScopedEntity> {
        match self.try_entity_ref(entity_id)? {
            QueryEngineEntityId::Engine(_) => Ok(&self.engine),
            QueryEngineEntityId::Worker(_) => Ok(self.workers.get(&entity_id).unwrap()),
            QueryEngineEntityId::QueryGroup(_) => Ok(self.query_groups.get(&entity_id).unwrap()),
            QueryEngineEntityId::Query(_) => Ok(self.queries.get(&entity_id).unwrap()),
            QueryEngineEntityId::Plan(_) => Ok(self.plans.get(&entity_id).unwrap()),
            QueryEngineEntityId::Operator(_) => Ok(self.operators.get(&entity_id).unwrap()),
            QueryEngineEntityId::Port(_) => Ok(self.ports.get(&entity_id).unwrap()),
        }
    }
}

impl ResourceCollection for InMemoryQueryEngineModel {
    fn resources(&self) -> impl Iterator<Item = &dyn Resource> {
        std::iter::empty()
    }

    fn resource_groups(&self) -> impl Iterator<Item = &dyn ResourceGroup> {
        std::iter::once(&self.engine as &dyn ResourceGroup)
            .chain(
                self.workers
                    .values()
                    .map(|entity| entity as &dyn ResourceGroup),
            )
            .chain(
                self.query_groups
                    .values()
                    .map(|entity| entity as &dyn ResourceGroup),
            )
            .chain(
                self.queries
                    .values()
                    .map(|entity| entity as &dyn ResourceGroup),
            )
            .chain(
                self.plans
                    .values()
                    .map(|entity| entity as &dyn ResourceGroup),
            )
            .chain(
                self.operators
                    .values()
                    .map(|entity| entity as &dyn ResourceGroup),
            )
            .chain(
                self.ports
                    .values()
                    .map(|entity| entity as &dyn ResourceGroup),
            )
    }

    fn resource(&self, id: Uuid) -> AnalyzerResult<&dyn Resource> {
        Err(AnalyzerError::InvalidId(id))
    }

    fn resource_type(&self, name: &str) -> AnalyzerResult<&ResourceTypeDecl> {
        Err(AnalyzerError::InvalidTypeName(name.to_owned()))
    }

    fn resource_group(&self, id: Uuid) -> AnalyzerResult<&dyn ResourceGroup> {
        match self.try_entity_ref(id)? {
            QueryEngineEntityId::Engine(_) => Ok(&self.engine),
            QueryEngineEntityId::Worker(_) => Ok(self.workers.get(&id).unwrap()),
            QueryEngineEntityId::QueryGroup(_) => Ok(self.query_groups.get(&id).unwrap()),
            QueryEngineEntityId::Query(_) => Ok(self.queries.get(&id).unwrap()),
            QueryEngineEntityId::Plan(_) => Ok(self.plans.get(&id).unwrap()),
            QueryEngineEntityId::Operator(_) => Ok(self.operators.get(&id).unwrap()),
            QueryEngineEntityId::Port(_) => Ok(self.ports.get(&id).unwrap()),
        }
    }

    fn resource_group_child_groups(&self, id: Uuid) -> AnalyzerResult<impl Iterator<Item = Uuid>> {
        self.resource_group(id)?;
        Ok(self
            .resource_groups()
            .filter_map(move |group| (group.parent_group_id() == Some(id)).then_some(group.id())))
    }

    fn resource_group_child_resources(
        &self,
        id: Uuid,
    ) -> AnalyzerResult<impl Iterator<Item = Uuid>> {
        self.resource_group(id)?;
        Ok(std::iter::empty())
    }
}

/// Builds an [`InMemoryQueryEngineModel`] from normalized query-engine events.
pub struct InMemoryQueryEngineModelBuilder {
    engine: Engine,
    workers: HashMap<Uuid, Worker>,
    query_groups: HashMap<Uuid, QueryGroup>,
    queries: HashMap<Uuid, QueryBuilder>,
    plans: HashMap<Uuid, Plan>,
    operators: HashMap<Uuid, Operator>,
    ports: HashMap<Uuid, Port>,
}

impl InMemoryQueryEngineModelBuilder {
    pub fn try_new(engine_id: Uuid) -> AnalyzerResult<Self> {
        Ok(Self {
            engine: Engine::try_new(engine_id)?,
            workers: HashMap::default(),
            query_groups: HashMap::default(),
            queries: HashMap::default(),
            plans: HashMap::default(),
            operators: HashMap::default(),
            ports: HashMap::default(),
        })
    }

    pub fn push_engine(&mut self, event: Event<EngineEvent>) -> AnalyzerResult<()> {
        if event.id != self.engine.id() {
            return Err(AnalyzerError::Validation(format!(
                "multiple engine instances in one model: expected {}, found {}",
                self.engine.id(),
                event.id
            )));
        }
        self.engine.push(event)
    }

    pub fn push_worker(&mut self, event: Event<WorkerEvent>) -> AnalyzerResult<()> {
        let worker = match self.workers.entry(event.id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Worker::try_new(event.id)?)
            }
        };
        worker.push(event)
    }

    pub fn push_query_group(&mut self, event: Event<QueryGroupEvent>) -> AnalyzerResult<()> {
        let group = match self.query_groups.entry(event.id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(QueryGroup::try_new(event.id)?)
            }
        };
        group.push(event)
    }

    pub fn push_query(&mut self, event: Event<QueryEvent>) -> AnalyzerResult<()> {
        let query = match self.queries.entry(event.id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(QueryBuilder::try_new(event.id)?)
            }
        };
        query.push_transition(event);
        Ok(())
    }

    pub fn push_plan(&mut self, event: Event<PlanEvent>) -> AnalyzerResult<()> {
        let plan = match self.plans.entry(event.id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Plan::try_new(event.id)?)
            }
        };
        plan.push(event)
    }

    pub fn push_operator(&mut self, event: Event<OperatorEvent>) -> AnalyzerResult<()> {
        let operator = match self.operators.entry(event.id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Operator::try_new(event.id)?)
            }
        };
        operator.push(event)
    }

    pub fn push_port(&mut self, event: Event<PortEvent>) -> AnalyzerResult<()> {
        let port = match self.ports.entry(event.id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(Port::try_new(event.id)?)
            }
        };
        port.push(event)
    }

    /// Ingests one normalized query-engine event.
    pub fn try_push(&mut self, event: Event<QueryEngineEvent>) -> AnalyzerResult<()> {
        let Event {
            id,
            timestamp,
            data,
        } = event;
        match data {
            QueryEngineEvent::Engine(data) => self.push_engine(Event::new(id, timestamp, data)),
            QueryEngineEvent::Worker(data) => self.push_worker(Event::new(id, timestamp, data)),
            QueryEngineEvent::QueryGroup(data) => {
                self.push_query_group(Event::new(id, timestamp, data))
            }
            QueryEngineEvent::Query(data) => self.push_query(Event::new(id, timestamp, data)),
            QueryEngineEvent::Plan(data) => self.push_plan(Event::new(id, timestamp, data)),
            QueryEngineEvent::Operator(data) => self.push_operator(Event::new(id, timestamp, data)),
            QueryEngineEvent::Port(data) => self.push_port(Event::new(id, timestamp, data)),
        }
    }

    /// Ingests all normalized events from `events` in iterator order.
    pub fn try_extend(
        &mut self,
        events: impl IntoIterator<Item = Event<QueryEngineEvent>>,
    ) -> AnalyzerResult<()> {
        for event in events {
            self.try_push(event)?;
        }
        Ok(())
    }

    pub fn try_build(self) -> AnalyzerResult<InMemoryQueryEngineModel> {
        if self.engine.0.earliest_timestamp().is_none() {
            return Err(AnalyzerError::IncompleteEntity(format!(
                "engine {} has no events",
                self.engine.id()
            )));
        }
        let queries = self
            .queries
            .into_iter()
            .map(|(id, builder)| Ok((id, Query::from_builder(builder)?)))
            .collect::<AnalyzerResult<_>>()?;
        Ok(InMemoryQueryEngineModel {
            engine: self.engine,
            workers: self.workers,
            query_groups: self.query_groups,
            queries,
            plans: self.plans,
            operators: self.operators,
            ports: self.ports,
        })
    }
}

/// Query-scoped view borrowing entities from an [`InMemoryQueryEngineModel`].
pub struct InMemoryQueryEngineModelView<'a> {
    engine: &'a Engine,
    query_group: &'a QueryGroup,
    query: &'a Query,
    workers: HashMap<Uuid, &'a Worker>,
    plans: HashMap<Uuid, &'a Plan>,
    operators: HashMap<Uuid, &'a Operator>,
    ports: HashMap<Uuid, &'a Port>,
}

impl<'a> InMemoryQueryEngineModelView<'a> {
    pub fn try_new(model: &'a InMemoryQueryEngineModel, query_id: Uuid) -> AnalyzerResult<Self> {
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
        Ok(Self {
            engine: &model.engine,
            query_group,
            query,
            workers,
            plans,
            operators,
            ports,
        })
    }
}

impl Model for InMemoryQueryEngineModelView<'_> {
    type EntityIdType = QueryEngineEntityId;

    fn try_entity_ref(&self, id: Uuid) -> AnalyzerResult<Self::EntityIdType> {
        if self.engine.id() == id {
            Ok(QueryEngineEntityId::Engine(id))
        } else if self.workers.contains_key(&id) {
            Ok(QueryEngineEntityId::Worker(id))
        } else if self.query_group.id() == id {
            Ok(QueryEngineEntityId::QueryGroup(id))
        } else if self.query.id() == id {
            Ok(QueryEngineEntityId::Query(id))
        } else if self.plans.contains_key(&id) {
            Ok(QueryEngineEntityId::Plan(id))
        } else if self.operators.contains_key(&id) {
            Ok(QueryEngineEntityId::Operator(id))
        } else if self.ports.contains_key(&id) {
            Ok(QueryEngineEntityId::Port(id))
        } else {
            Err(AnalyzerError::InvalidId(id))
        }
    }

    fn root(&self) -> AnalyzerResult<&impl ResourceGroup> {
        Ok(self.engine)
    }
}

impl QueryEngineModel for InMemoryQueryEngineModelView<'_> {
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
    fn query(&self, id: Uuid) -> AnalyzerResult<&Query> {
        (self.query.id() == id)
            .then_some(self.query)
            .ok_or(AnalyzerError::InvalidId(id))
    }
    fn query_group(&self, id: Uuid) -> AnalyzerResult<&QueryGroup> {
        (self.query_group.id() == id)
            .then_some(self.query_group)
            .ok_or(AnalyzerError::InvalidId(id))
    }
    fn worker(&self, id: Uuid) -> AnalyzerResult<&Worker> {
        self.workers
            .get(&id)
            .copied()
            .ok_or(AnalyzerError::InvalidId(id))
    }
    fn plan(&self, id: Uuid) -> AnalyzerResult<&Plan> {
        self.plans
            .get(&id)
            .copied()
            .ok_or(AnalyzerError::InvalidId(id))
    }
    fn operator(&self, id: Uuid) -> AnalyzerResult<&Operator> {
        self.operators
            .get(&id)
            .copied()
            .ok_or(AnalyzerError::InvalidId(id))
    }
    fn port(&self, id: Uuid) -> AnalyzerResult<&Port> {
        self.ports
            .get(&id)
            .copied()
            .ok_or(AnalyzerError::InvalidId(id))
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

impl ScopeCollection for InMemoryQueryEngineModelView<'_> {
    fn scoped_entities(&self) -> impl Iterator<Item = &dyn ScopedEntity> {
        std::iter::once(self.engine as &dyn ScopedEntity)
            .chain(
                self.workers
                    .values()
                    .map(|entity| *entity as &dyn ScopedEntity),
            )
            .chain(std::iter::once(self.query_group as &dyn ScopedEntity))
            .chain(std::iter::once(self.query as &dyn ScopedEntity))
            .chain(
                self.plans
                    .values()
                    .map(|entity| *entity as &dyn ScopedEntity),
            )
            .chain(
                self.operators
                    .values()
                    .map(|entity| *entity as &dyn ScopedEntity),
            )
            .chain(
                self.ports
                    .values()
                    .map(|entity| *entity as &dyn ScopedEntity),
            )
    }

    fn scoped_entity(&self, entity_id: Uuid) -> AnalyzerResult<&dyn ScopedEntity> {
        match self.try_entity_ref(entity_id)? {
            QueryEngineEntityId::Engine(_) => Ok(self.engine),
            QueryEngineEntityId::Worker(_) => Ok(*self.workers.get(&entity_id).unwrap()),
            QueryEngineEntityId::QueryGroup(_) => Ok(self.query_group),
            QueryEngineEntityId::Query(_) => Ok(self.query),
            QueryEngineEntityId::Plan(_) => Ok(*self.plans.get(&entity_id).unwrap()),
            QueryEngineEntityId::Operator(_) => Ok(*self.operators.get(&entity_id).unwrap()),
            QueryEngineEntityId::Port(_) => Ok(*self.ports.get(&entity_id).unwrap()),
        }
    }
}

impl ResourceCollection for InMemoryQueryEngineModelView<'_> {
    fn resources(&self) -> impl Iterator<Item = &dyn Resource> {
        std::iter::empty()
    }
    fn resource_groups(&self) -> impl Iterator<Item = &dyn ResourceGroup> {
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
    fn resource(&self, id: Uuid) -> AnalyzerResult<&dyn Resource> {
        Err(AnalyzerError::InvalidId(id))
    }
    fn resource_type(&self, name: &str) -> AnalyzerResult<&ResourceTypeDecl> {
        Err(AnalyzerError::InvalidTypeName(name.to_owned()))
    }
    fn resource_group(&self, id: Uuid) -> AnalyzerResult<&dyn ResourceGroup> {
        match self.try_entity_ref(id)? {
            QueryEngineEntityId::Engine(_) => Ok(self.engine),
            QueryEngineEntityId::Worker(_) => Ok(*self.workers.get(&id).unwrap()),
            QueryEngineEntityId::QueryGroup(_) => Ok(self.query_group),
            QueryEngineEntityId::Query(_) => Ok(self.query),
            QueryEngineEntityId::Plan(_) => Ok(*self.plans.get(&id).unwrap()),
            QueryEngineEntityId::Operator(_) => Ok(*self.operators.get(&id).unwrap()),
            QueryEngineEntityId::Port(_) => Ok(*self.ports.get(&id).unwrap()),
        }
    }
    fn resource_group_child_groups(&self, id: Uuid) -> AnalyzerResult<impl Iterator<Item = Uuid>> {
        self.resource_group(id)?;
        Ok(self
            .resource_groups()
            .filter_map(move |group| (group.parent_group_id() == Some(id)).then_some(group.id())))
    }
    fn resource_group_child_resources(
        &self,
        id: Uuid,
    ) -> AnalyzerResult<impl Iterator<Item = Uuid>> {
        self.resource_group(id)?;
        Ok(std::iter::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: Uuid, timestamp: u64, data: QueryEngineEvent) -> Event<QueryEngineEvent> {
        Event::new(id, timestamp, data)
    }

    #[test]
    fn rejects_model_without_engine_events() {
        let engine_id = Uuid::from_u128(1);
        assert!(matches!(
            InMemoryQueryEngineModelBuilder::try_new(engine_id)
                .unwrap()
                .try_build(),
            Err(AnalyzerError::IncompleteEntity(_))
        ));
    }

    #[test]
    fn builds_reusable_query_engine_model_from_normalized_events() {
        let engine_id = Uuid::from_u128(1);
        let worker_id = Uuid::from_u128(2);
        let query_group_id = Uuid::from_u128(3);
        let query_id = Uuid::from_u128(4);
        let plan_id = Uuid::from_u128(5);
        let operator_id = Uuid::from_u128(6);
        let source_port_id = Uuid::from_u128(7);
        let target_port_id = Uuid::from_u128(8);

        let mut builder = InMemoryQueryEngineModelBuilder::try_new(engine_id).unwrap();
        builder
            .try_extend([
                event(
                    engine_id,
                    0,
                    QueryEngineEvent::Engine(EngineEvent::Init {
                        implementation: EngineImplementation {
                            name: Some("test".to_owned()),
                            version: None,
                            custom_attributes: DynamicAttributes::default(),
                        },
                        instance_name: Some("engine".to_owned()),
                    }),
                ),
                event(
                    worker_id,
                    1,
                    QueryEngineEvent::Worker(WorkerEvent::Init {
                        parent_engine_id: engine_id,
                        instance_name: "worker".to_owned(),
                    }),
                ),
                event(
                    query_group_id,
                    1,
                    QueryEngineEvent::QueryGroup(QueryGroupEvent {
                        instance_name: "group".to_owned(),
                        engine_id,
                    }),
                ),
                event(
                    query_id,
                    2,
                    QueryEngineEvent::Query(QueryEvent::Init {
                        seq: 0,
                        instance_name: "query".to_owned(),
                        query_group_id,
                    }),
                ),
                event(
                    query_id,
                    3,
                    QueryEngineEvent::Query(QueryEvent::Planning { seq: 1 }),
                ),
                event(
                    query_id,
                    4,
                    QueryEngineEvent::Query(QueryEvent::Executing { seq: 2 }),
                ),
                event(
                    plan_id,
                    4,
                    QueryEngineEvent::Plan(PlanEvent {
                        parent: PlanParent {
                            query_id,
                            plan_id: None,
                        },
                        instance_name: "plan".to_owned(),
                        edges: vec![Edge {
                            source: source_port_id,
                            target: target_port_id,
                        }],
                        worker_id: Some(worker_id),
                    }),
                ),
                event(
                    operator_id,
                    4,
                    QueryEngineEvent::Operator(OperatorEvent::Declaration {
                        plan_id,
                        parent_operator_ids: Vec::new(),
                        instance_name: "operator".to_owned(),
                        type_name: "scan".to_owned(),
                        custom_attributes: DynamicAttributes::default(),
                    }),
                ),
                event(
                    source_port_id,
                    4,
                    QueryEngineEvent::Port(PortEvent::Declaration {
                        operator_id,
                        instance_name: "out".to_owned(),
                    }),
                ),
                event(
                    target_port_id,
                    4,
                    QueryEngineEvent::Port(PortEvent::Declaration {
                        operator_id,
                        instance_name: "in".to_owned(),
                    }),
                ),
                event(
                    query_id,
                    5,
                    QueryEngineEvent::Query(QueryEvent::Done { seq: 3 }),
                ),
                event(worker_id, 6, QueryEngineEvent::Worker(WorkerEvent::Exit)),
                event(engine_id, 6, QueryEngineEvent::Engine(EngineEvent::Exit)),
            ])
            .unwrap();

        let model = builder.try_build().unwrap();
        assert_eq!(model.workers().count(), 1);
        assert_eq!(model.queries().count(), 1);
        assert_eq!(model.plans().count(), 1);
        assert_eq!(model.operators().count(), 1);
        assert_eq!(model.ports().count(), 2);
        assert_eq!(model.engine().unwrap().scope_id(), None);
        assert_eq!(model.worker(worker_id).unwrap().scope_id(), Some(engine_id));
        assert_eq!(
            model.query_group(query_group_id).unwrap().scope_id(),
            Some(engine_id)
        );
        assert_eq!(
            model.query(query_id).unwrap().scope_id(),
            Some(query_group_id)
        );
        assert_eq!(model.plan(plan_id).unwrap().scope_id(), Some(query_id));
        assert_eq!(
            model.operator(operator_id).unwrap().scope_id(),
            Some(plan_id)
        );
        assert_eq!(
            model.port(source_port_id).unwrap().scope_id(),
            Some(operator_id)
        );
        assert_eq!(model.scoped_entities().count(), 8);
        assert_eq!(
            model
                .scope_children(engine_id)
                .map(Entity::id)
                .collect::<std::collections::HashSet<_>>(),
            std::collections::HashSet::from([worker_id, query_group_id])
        );
        assert_eq!(
            model
                .scope_children(query_id)
                .map(Entity::id)
                .collect::<Vec<_>>(),
            [plan_id]
        );
        assert_eq!(
            model
                .scope_children(plan_id)
                .map(Entity::id)
                .collect::<Vec<_>>(),
            [operator_id]
        );
        assert_eq!(
            model
                .scope_children(operator_id)
                .map(Entity::id)
                .collect::<std::collections::HashSet<_>>(),
            std::collections::HashSet::from([source_port_id, target_port_id])
        );
        assert_eq!(
            model.scoped_entity(target_port_id).unwrap().id(),
            target_port_id
        );
        assert_eq!(
            model.plan(plan_id).unwrap().edges().collect::<Vec<_>>(),
            vec![(source_port_id, target_port_id)]
        );

        let view = model.query_view(query_id).unwrap();
        assert_eq!(
            view.query(query_id)
                .unwrap()
                .to_ui()
                .unwrap()
                .instance_name
                .as_deref(),
            Some("query")
        );
        assert_eq!(view.workers().count(), 1);
        assert_eq!(view.operators().count(), 1);
        assert_eq!(view.scoped_entities().count(), 8);
        assert_eq!(
            view.scope_children(engine_id)
                .map(Entity::id)
                .collect::<std::collections::HashSet<_>>(),
            std::collections::HashSet::from([worker_id, query_group_id])
        );
        assert_eq!(
            view.scoped_entity(target_port_id).unwrap().id(),
            target_port_id
        );
    }
}
