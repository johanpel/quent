// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use clap::Parser;
use petgraph::{Directed, Direction, Graph, graph::NodeIndex, visit::EdgeRef};
use quent_dynamic_attributes::{DynamicAttribute, DynamicList, DynamicStruct};
use quent_io::clap::ExporterArgs;
use quent_simulator_instrumentation as instr;
use rand::{RngExt, distr::slice::Choose, rng};
use tracing::{debug, info};
use uuid::Uuid;

type SimulatorContext = instr::Context<instr::Simulator>;

#[derive(Parser, Debug)]
#[command(name = "simulator")]
#[command(about = "Emits simulated query engine telemetry", long_about = None)]
struct Args {
    /// Number of query groups
    #[arg(long, default_value_t = 1)]
    num_query_groups: usize,

    /// Number of queries per query group
    #[arg(long, default_value_t = 1)]
    num_queries: usize,

    /// Number of tasks per operator
    #[arg(long, default_value_t = 32)]
    num_tasks: usize,

    /// Number of workers
    #[arg(long, default_value_t = 2)]
    num_workers: usize,

    /// Number of threads per worker thread pool
    #[arg(long, default_value_t = 2)]
    num_threads: usize,

    #[command(flatten)]
    exporter: ExporterArgs,
}

fn initialize_tracing() {
    tracing_subscriber::fmt()
        .with_target(true)
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .init();
}

fn log_resource_links(engine_id: Uuid, query_id: Uuid, resource_id: Uuid, resource_name: &str) {
    debug!("\tResource: {resource_name}
\t\tTimeline: http://localhost:8080/analyzer/engine/{engine_id}/query/{query_id}/resource/{resource_id}/timeline?num_bins=16&start=0&end=4"
    );
}

fn log_resource_group_links(
    engine_id: Uuid,
    query_id: Uuid,
    resource_group_id: Uuid,
    resource_group_name: &str,
) {
    debug!("\tResource Group: {resource_group_name}
\t\tTimeline: http://localhost:8080/analyzer/engine/{engine_id}/query/{query_id}/resource_group/{resource_group_id}/timeline?num_bins=16&start=0&end=4"
    );
}

fn sleep_short() {
    std::thread::sleep(Duration::from_micros(1));
}

fn sleep_long() {
    std::thread::sleep(Duration::from_micros(25));
}

fn sleep_sometimes_really_long() {
    // make 1% tasks incredibly slow
    std::thread::sleep(Duration::from_micros(if rng().random_ratio(1, 100) {
        50000
    } else {
        25
    }));
}

struct Operator<T: Debug> {
    handle: instr::Handle<instr::Operator>,
    parents: Vec<instr::EntityRef<instr::Operator>>,
    kind: T,
    tasks_processed: AtomicU64,
}

impl<T> Operator<T>
where
    T: Debug,
{
    fn name(&self) -> String {
        format!("{:?}", self.kind)
    }

    fn new(
        context: &SimulatorContext,
        kind: T,
        parents: Vec<instr::EntityRef<instr::Operator>>,
    ) -> Self {
        Self {
            handle: context.observer::<instr::Operator>().handle(),
            parents,
            kind,
            tasks_processed: AtomicU64::new(0),
        }
    }
}

impl<T> Display for Operator<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

struct Port {
    handle: instr::Handle<instr::Port>,
    name: &'static str,
    num_bytes: AtomicU64,
    num_rows: AtomicU64,
}

struct Edge {
    source: Port,
    target: Port,
}

impl Display for Edge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -> {}", self.source.name, self.target.name)
    }
}

impl Edge {
    fn new(context: &SimulatorContext, source: &'static str, target: &'static str) -> Edge {
        let port_obs = context.observer::<instr::Port>();
        Edge {
            source: Port {
                handle: port_obs.handle(),
                name: source,
                num_bytes: AtomicU64::new(0),
                num_rows: AtomicU64::new(0),
            },
            target: Port {
                handle: port_obs.handle(),
                name: target,
                num_bytes: AtomicU64::new(0),
                num_rows: AtomicU64::new(0),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Logical {
    Scan,
    Project,
    Join,
    Sort,
    Limit,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Physical {
    FileSystemScan,
    JoinPartition,
    JoinLocal,
    Sort,
    Limit,
    Output,
}

struct Plan<T>
where
    T: Debug,
{
    handle: instr::Handle<instr::Plan>,
    name: String,
    query: instr::EntityRef<instr::Query>,
    parent_plan: Option<instr::EntityRef<instr::Plan>>,
    dag: Graph<Operator<T>, Edge, Directed>,
    execute: bool,
}

impl<T: Debug> Plan<T> {
    pub fn declare(&mut self, worker: Option<instr::EntityRef<instr::Worker>>) {
        self.handle
            .declaration(
                instr::PlanParent {
                    query_id: self.query.clone(),
                    plan_id: self.parent_plan.clone(),
                },
                self.name.clone(),
                self.dag
                    .edge_references()
                    .map(|edge| instr::Edge {
                        source: edge.weight().source.handle.as_entity_ref(),
                        target: edge.weight().target.handle.as_entity_ref(),
                    })
                    .collect(),
                worker,
            )
            .unwrap();

        for node_idx in self.dag.node_indices().collect::<Vec<_>>() {
            let op = &mut self.dag[node_idx];
            op.handle
                .declaration(
                    self.handle.as_entity_ref(),
                    op.parents.clone(),
                    format!("{}-{node_idx:?}", op.name()),
                    op.name(),
                    Default::default(),
                )
                .unwrap();
        }

        for edge_idx in self.dag.edge_indices().collect::<Vec<_>>() {
            let (source_idx, target_idx) = self.dag.edge_endpoints(edge_idx).unwrap();
            let source_operator = self.dag[source_idx].handle.as_entity_ref();
            let target_operator = self.dag[target_idx].handle.as_entity_ref();
            let edge = &mut self.dag[edge_idx];
            edge.source
                .handle
                .declaration(source_operator, edge.source.name.to_string())
                .unwrap();
            edge.target
                .handle
                .declaration(target_operator, edge.target.name.to_string())
                .unwrap();
        }
    }
}

// Create the following logical plan:
// Scan -> Project \
//                  -> Join -> Sort -> Limit -> Output
// Scan -> Project /
fn make_logical_plan(
    context: &SimulatorContext,
    query: instr::EntityRef<instr::Query>,
    name: String,
) -> Plan<Logical> {
    // Add a scan --> project branch and return the (project, project output port) Uuids.
    fn add_scan_project_branch(
        context: &SimulatorContext,
        plan: &mut Graph<Operator<Logical>, Edge, Directed>,
    ) -> NodeIndex {
        let scan = plan.add_node(Operator::new(context, Logical::Scan, vec![]));
        let project = plan.add_node(Operator::new(context, Logical::Project, vec![]));
        plan.add_edge(scan, project, Edge::new(context, "out", "in"));

        project
    }

    let mut dag = Graph::new();

    let project_a = add_scan_project_branch(context, &mut dag);
    let project_b = add_scan_project_branch(context, &mut dag);

    let join = dag.add_node(Operator::new(context, Logical::Join, vec![]));
    dag.add_edge(project_a, join, Edge::new(context, "out", "left"));
    dag.add_edge(project_b, join, Edge::new(context, "out", "right"));

    let sort = dag.add_node(Operator::new(context, Logical::Sort, vec![]));
    dag.add_edge(join, sort, Edge::new(context, "out", "in"));

    let limit = dag.add_node(Operator::new(context, Logical::Limit, vec![]));
    dag.add_edge(sort, limit, Edge::new(context, "out", "in"));

    let output = dag.add_node(Operator::new(context, Logical::Output, vec![]));
    dag.add_edge(limit, output, Edge::new(context, "out", "in"));

    Plan {
        handle: context.observer::<instr::Plan>().handle(),
        name,
        query,
        parent_plan: None,
        dag,
        execute: false,
    }
}

fn simulate_planning(context: &SimulatorContext, logical: &Plan<Logical>) -> Plan<Physical> {
    // Find the output node
    let output = logical
        .dag
        .node_indices()
        .collect::<Vec<_>>()
        .into_iter()
        .find(|n| logical.dag[*n].kind == Logical::Output)
        .unwrap();

    // Build a physical plan
    let mut physical = Plan {
        handle: context.observer::<instr::Plan>().handle(),
        name: "physical".into(),
        query: logical.query.clone(),
        parent_plan: Some(logical.handle.as_entity_ref()),
        dag: Graph::new(),
        execute: true,
    };

    lower_logical(context, logical, &mut physical, output, None);

    physical
}

fn lower_logical(
    context: &SimulatorContext,
    logical: &Plan<Logical>,
    physical: &mut Plan<Physical>,
    logical_current_idx: NodeIndex,
    physical_target_idx_port: Option<(NodeIndex, &'static str)>,
) {
    let current_logical_op = &logical.dag[logical_current_idx];

    match current_logical_op.kind {
        Logical::Scan => {
            unimplemented!("this shouldn't happen in this simulator, yet")
        }
        Logical::Project => {
            // Check whether this project has an incoming scan source to simulate predicate pushdown
            if let Some(scan_edge) = logical
                .dag
                .edges_directed(logical_current_idx, Direction::Incoming)
                .find(|edge| logical.dag[edge.source()].kind == Logical::Scan)
            {
                let scan_op = &logical.dag[scan_edge.source()];
                let source = physical.dag.add_node(Operator::new(
                    context,
                    Physical::FileSystemScan,
                    vec![
                        current_logical_op.handle.as_entity_ref(),
                        scan_op.handle.as_entity_ref(),
                    ],
                ));
                if let Some((target_node, target_port)) = physical_target_idx_port {
                    physical.dag.add_edge(
                        source,
                        target_node,
                        Edge::new(context, target_port, "in"),
                    );
                }
            } else {
                unimplemented!("this shouldn't happen in this simulator, yet");
            }
        }
        Logical::Join => {
            // split up in a partition stage and join stage
            let partition = physical.dag.add_node(Operator::new(
                context,
                Physical::JoinPartition,
                vec![current_logical_op.handle.as_entity_ref()],
            ));
            let local = physical.dag.add_node(Operator::new(
                context,
                Physical::JoinLocal,
                vec![current_logical_op.handle.as_entity_ref()],
            ));
            physical.dag.add_edge(
                partition,
                local,
                Edge::new(context, "build_out", "build_in"),
            );
            physical.dag.add_edge(
                partition,
                local,
                Edge::new(context, "probe_out", "probe_in"),
            );

            if let Some((target_node, target_port)) = physical_target_idx_port {
                physical
                    .dag
                    .add_edge(local, target_node, Edge::new(context, "out", target_port));
            }

            // Recurse up both branches
            for input_edge in logical
                .dag
                .edges_directed(logical_current_idx, Direction::Incoming)
            {
                lower_logical(
                    context,
                    logical,
                    physical,
                    input_edge.source(),
                    Some((partition, input_edge.weight().target.name)),
                );
            }
        }
        Logical::Sort => {
            let sort = physical.dag.add_node(Operator::new(
                context,
                Physical::Sort,
                vec![current_logical_op.handle.as_entity_ref()],
            ));
            if let Some((target_node, target_port)) = physical_target_idx_port {
                physical
                    .dag
                    .add_edge(sort, target_node, Edge::new(context, "out", target_port));
            }
            let input_edge = logical
                .dag
                .edges_directed(logical_current_idx, Direction::Incoming)
                .next()
                .unwrap();
            lower_logical(
                context,
                logical,
                physical,
                input_edge.source(),
                Some((sort, input_edge.weight().target.name)),
            );
        }
        Logical::Limit => {
            let limit = physical.dag.add_node(Operator::new(
                context,
                Physical::Limit,
                vec![current_logical_op.handle.as_entity_ref()],
            ));
            if let Some((target_node, target_port)) = physical_target_idx_port {
                physical
                    .dag
                    .add_edge(limit, target_node, Edge::new(context, "out", target_port));
            }
            let input_edge = logical
                .dag
                .edges_directed(logical_current_idx, Direction::Incoming)
                .next()
                .unwrap();
            lower_logical(
                context,
                logical,
                physical,
                input_edge.source(),
                Some((limit, input_edge.weight().target.name)),
            );
        }
        Logical::Output => {
            let output = physical.dag.add_node(Operator::new(
                context,
                Physical::Output,
                vec![current_logical_op.handle.as_entity_ref()],
            ));
            if let Some((target_node, target_port)) = physical_target_idx_port {
                physical
                    .dag
                    .add_edge(output, target_node, Edge::new(context, "out", target_port));
            }
            let input_edge = logical
                .dag
                .edges_directed(logical_current_idx, Direction::Incoming)
                .next()
                .unwrap();
            lower_logical(
                context,
                logical,
                physical,
                input_edge.source(),
                Some((output, input_edge.weight().target.name)),
            );
        }
    }
}

struct Worker {
    handle: instr::Handle<instr::Worker>,
    memory: instr::Handle<instr::Memory>,
    filesystem: instr::Handle<instr::Memory>,
    fs_to_mem: instr::Handle<instr::StorageChannel>,
    mem_to_fs: instr::Handle<instr::StorageChannel>,
    thread_pool: instr::Handle<instr::ThreadPool>,
    threads: Vec<instr::Handle<instr::Processor>>,
}

impl Worker {
    fn new(
        name: String,
        context: &SimulatorContext,
        parent_engine: instr::EntityRef<instr::Engine>,
        num_threads: usize,
    ) -> Self {
        let worker_obs = context.observer::<instr::Worker>();

        info!("Spawning worker {name}");
        let mut handle = worker_obs.handle();
        handle.init(parent_engine, name.clone()).unwrap();

        let mem_obs = context.observer::<instr::Memory>();
        let ch_obs = context.observer::<instr::StorageChannel>();
        let proc_obs = context.observer::<instr::Processor>();

        // Filesystem
        let mut filesystem = mem_obs.handle();
        filesystem
            .initializing("Filesystem".to_string(), handle.as_entity_ref())
            .unwrap();
        filesystem
            .operating(instr::MemoryBounds { bytes: 0 })
            .unwrap();

        // Memory pool
        let mut memory = mem_obs.handle();
        memory
            .initializing("Memory".to_string(), handle.as_entity_ref())
            .unwrap();
        memory.operating(instr::MemoryBounds { bytes: 0 }).unwrap();

        // Filesystem -> Memory channel
        let mut fs_to_mem = ch_obs.handle();
        fs_to_mem
            .initializing(
                "Filesystem -> Memory".to_string(),
                handle.as_entity_ref(),
                filesystem.as_entity_ref(),
                memory.as_entity_ref(),
            )
            .unwrap();
        fs_to_mem
            .operating(instr::StorageChannelBounds { bytes: 0 })
            .unwrap();

        // Memory -> Filesystem channel
        let mut mem_to_fs = ch_obs.handle();
        mem_to_fs
            .initializing(
                "Memory -> Filesystem".to_string(),
                handle.as_entity_ref(),
                memory.as_entity_ref(),
                filesystem.as_entity_ref(),
            )
            .unwrap();
        mem_to_fs
            .operating(instr::StorageChannelBounds { bytes: 0 })
            .unwrap();

        // Thread pool
        let tp_obs = context.observer::<instr::ThreadPool>();
        let mut thread_pool = tp_obs.handle();
        thread_pool
            .declaration("Thread Pool".to_string(), handle.as_entity_ref())
            .unwrap();

        let mut threads = Vec::new();
        for index in 0..num_threads {
            let mut thread = proc_obs.handle();
            thread
                .initializing(format!("Thread {index}"), thread_pool.as_entity_ref())
                .unwrap();
            thread.operating().unwrap();
            threads.push(thread);
        }

        Self {
            handle,
            memory,
            filesystem,
            fs_to_mem,
            mem_to_fs,
            thread_pool,
            threads,
        }
    }

    fn execute_physical_operator_task(
        &self,
        context: &SimulatorContext,
        _index: usize,
        engine: &Engine,
        operator: &Operator<Physical>,
        thread: &instr::Handle<instr::Processor>,
    ) {
        let task_obs = context.observer::<instr::Task>();
        let mut task = task_obs.handle();
        task.queueing(operator.handle.as_entity_ref()).unwrap();

        sleep_long();
        let (spill, load, send) = match operator.kind {
            Physical::FileSystemScan => (false, rng().random_bool(0.5), false),
            Physical::JoinPartition => (false, rng().random_bool(0.5), true),
            Physical::JoinLocal => (true, rng().random_bool(0.5), false),
            Physical::Sort => (false, rng().random_bool(0.5), false),
            Physical::Limit => (false, rng().random_bool(0.5), false),
            Physical::Output => (false, rng().random_bool(0.5), false),
        };

        let num_bytes = rng().random_range(0..1024) * 1024 * 1024;

        task.allocating(thread.as_entity_ref_with(instr::ProcessorUsage))
            .unwrap();
        sleep_short();

        if spill {
            task.spilling(
                thread.as_entity_ref_with(instr::ProcessorUsage),
                self.mem_to_fs
                    .as_entity_ref_with(instr::StorageChannelUsage { bytes: num_bytes }),
            )
            .unwrap();
            sleep_sometimes_really_long();
            task.allocating(thread.as_entity_ref_with(instr::ProcessorUsage))
                .unwrap();
            sleep_short();
        }

        if load {
            task.loading(
                thread.as_entity_ref_with(instr::ProcessorUsage),
                self.fs_to_mem
                    .as_entity_ref_with(instr::StorageChannelUsage { bytes: num_bytes }),
                self.memory.as_entity_ref_with(instr::MemoryUsage {
                    bytes: rng().random_range(0..4) * num_bytes,
                }),
            )
            .unwrap();
            sleep_sometimes_really_long();
        }

        task.computing(
            num_bytes,
            thread.as_entity_ref_with(instr::ProcessorUsage),
            self.memory.as_entity_ref_with(instr::MemoryUsage {
                bytes: rng().random_range(0..4) * num_bytes,
            }),
        )
        .unwrap();

        if send {
            let worker_id = self.handle.uuid();
            let other_workers = engine.workers.keys().filter(|w| **w != worker_id);

            for other in other_workers {
                let link = engine.network_links.get(&(worker_id, *other)).unwrap();

                task.sending(
                    thread.as_entity_ref_with(instr::ProcessorUsage),
                    link.as_entity_ref_with(instr::NetworkChannelUsage { bytes: num_bytes }),
                )
                .unwrap();
                sleep_long();
            }
        }

        task.exit().unwrap();
    }

    fn execute_logical_plan(
        &self,
        context: &SimulatorContext,
        engine: &Engine,
        l_plan: &Plan<Logical>,
        num_tasks: usize,
    ) {
        let mut physical_plan = simulate_planning(context, l_plan);
        physical_plan.declare(Some(self.handle.as_entity_ref()));

        // Log analyzer debug links:
        let engine_id = engine.handle.uuid();
        let query_id = physical_plan.query.target;
        log_resource_links(engine_id, query_id, self.memory.uuid(), "Memory");
        log_resource_links(engine_id, query_id, self.filesystem.uuid(), "Filesystem");
        log_resource_links(
            engine_id,
            query_id,
            self.fs_to_mem.uuid(),
            "Filesystem -> Memory",
        );
        log_resource_links(
            engine_id,
            query_id,
            self.mem_to_fs.uuid(),
            "Memory -> Filesystem",
        );
        log_resource_group_links(engine_id, query_id, self.thread_pool.uuid(), "Thread Pool");
        for (index, thread) in self.threads.iter().enumerate() {
            log_resource_links(
                engine_id,
                query_id,
                thread.uuid(),
                format!("Thread {index}").as_str(),
            );
        }

        let nodes = petgraph::algo::toposort(&physical_plan.dag, None).unwrap();
        info!(
            "Topological order: {:?}",
            nodes
                .iter()
                .map(|node| format!("{:?}: {:?}", node, physical_plan.dag[*node].kind))
                .collect::<Vec<_>>()
        );

        if physical_plan.execute {
            // On each thread, run a bunch of tasks for each operator.
            let tasks_per_thread_per_op = num_tasks / self.threads.len();
            let plan = &physical_plan;
            let nodes = &nodes;
            std::thread::scope(|s| {
                for (thread_index, thread) in self.threads.iter().enumerate() {
                    s.spawn({
                        move || {
                            for task_index in thread_index * tasks_per_thread_per_op
                                ..(thread_index + 1) * tasks_per_thread_per_op
                            {
                                for node_idx in nodes {
                                    let op = &plan.dag[*node_idx];
                                    self.execute_physical_operator_task(
                                        context, task_index, engine, op, thread,
                                    );
                                    op.tasks_processed.fetch_add(1, Ordering::Relaxed);
                                    let edges =
                                        plan.dag.edges_directed(*node_idx, Direction::Outgoing);
                                    for edge in edges {
                                        edge.weight().source.num_bytes.fetch_add(
                                            rng().random_range(1024..1024 * 1024),
                                            Ordering::Relaxed,
                                        );
                                        edge.weight().source.num_rows.fetch_add(
                                            rng().random_range(16..1024),
                                            Ordering::Relaxed,
                                        );
                                        edge.weight().target.num_bytes.fetch_add(
                                            rng().random_range(1024..128 * 1024),
                                            Ordering::Relaxed,
                                        );
                                        edge.weight().target.num_rows.fetch_add(
                                            rng().random_range(16..1024),
                                            Ordering::Relaxed,
                                        );
                                    }
                                }
                            }
                        }
                    });
                }
            });
        }

        // Set some stats
        macro_rules! attr {
            (u64 $name:expr, $range:expr) => { DynamicAttribute::u64($name, rng().random_range($range)) };
            (u32 $name:expr, $val:expr) => { DynamicAttribute::u32($name, $val) };
            (f64 $name:expr, $range:expr) => { DynamicAttribute::f64($name, rng().random_range($range)) };
            (str $name:expr, $val:expr) => { DynamicAttribute::string($name, $val) };
            (pick $name:expr, $($choice:expr),+) => {
                DynamicAttribute::string($name, *rng().sample(Choose::new(&[$($choice),+]).unwrap()))
            };
        }

        for node_idx in nodes.iter() {
            let op = &physical_plan.dag[*node_idx];
            let tasks_processed = op.tasks_processed.load(Ordering::Relaxed);

            // Common metrics for all operators
            let mut attributes = vec![
                DynamicAttribute::u64("tasks_processed", tasks_processed),
                attr!(u64 "wall_time_ns",       100_000..5_000_000_000),
                attr!(u64 "cpu_time_ns",        50_000..4_000_000_000),
                attr!(u64 "peak_memory_bytes",  1024..512 * 1024 * 1024),
                attr!(u64 "output_rows",        0..10_000_000),
                attr!(u64 "output_bytes",       0..2u64 * 1024 * 1024 * 1024),
                attr!(u64 "input_rows",         0..50_000_000),
                attr!(u64 "input_bytes",        0..4u64 * 1024 * 1024 * 1024),
                attr!(u64 "num_batches",        1..2048),
                attr!(f64 "avg_batch_rows",     64.0..65536.0),
            ];

            match op.kind {
                Physical::FileSystemScan => {
                    let num_files: u64 = rng().random_range(1..256);
                    attributes.extend([
                        attr!(str "file_name",              "/dev/null"),
                        DynamicAttribute::u64("files_scanned", num_files),
                        attr!(u64 "bytes_read",             1024..8u64 * 1024 * 1024 * 1024),
                        attr!(u64 "row_groups_read",        1..1024),
                        attr!(u64 "row_groups_skipped",     0..512),
                        attr!(u64 "pages_read",             1..8192),
                        attr!(u64 "pages_decompressed",     1..8192),
                        attr!(u64 "io_wait_ns",             10_000..2_000_000_000),
                        attr!(f64 "io_throughput_mbs",      50.0..6000.0),
                        attr!(u64 "decompress_time_ns",     10_000..500_000_000),
                        attr!(u64 "predicate_filter_time_ns", 0..100_000_000),
                        attr!(f64 "predicate_selectivity",  0.001..1.0),
                        attr!(u64 "null_count",             0..100_000),
                        attr!(u64 "columns_projected",      1..64),
                        // Per-file byte counts
                        DynamicAttribute::list(
                            "per_file_bytes_read",
                            DynamicList::U64(
                                (0..num_files)
                                    .map(|_| rng().random_range(1024..1024 * 1024 * 1024))
                                    .collect(),
                            ),
                        ),
                        // Column projection info
                        DynamicAttribute::list(
                            "projected_column_names",
                            DynamicList::String(
                                [
                                    "id", "name", "ts", "amount", "region", "status", "category",
                                    "score",
                                ]
                                .iter()
                                .take(rng().random_range(1..8))
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        ),
                    ]);
                }
                Physical::JoinPartition => {
                    let num_partitions: u64 = rng().random_range(2..256);
                    attributes.extend([
                        attr!(u64  "average_partition_size_bytes", 1..1024 * 1024 * 1024),
                        attr!(pick "join_strategy",          "broadcast", "hash partition"),
                        DynamicAttribute::u64("num_partitions", num_partitions),
                        attr!(u64  "partition_time_ns",      100_000..1_000_000_000),
                        attr!(u64  "hash_time_ns",           50_000..500_000_000),
                        attr!(f64  "partition_skew",         0.0..5.0),
                        attr!(u64  "max_partition_rows",     100..1_000_000),
                        attr!(u64  "min_partition_rows",     0..10_000),
                        attr!(u64  "build_side_bytes",       1024..2u64 * 1024 * 1024 * 1024),
                        attr!(u64  "probe_side_bytes",       1024..4u64 * 1024 * 1024 * 1024),
                        attr!(u64  "network_bytes_sent",     0..2u64 * 1024 * 1024 * 1024),
                        attr!(u64  "network_time_ns",        0..2_000_000_000),
                        // Row count per partition
                        DynamicAttribute::list(
                            "partition_row_counts",
                            DynamicList::U64(
                                (0..num_partitions)
                                    .map(|_| rng().random_range(0..1_000_000))
                                    .collect(),
                            ),
                        ),
                    ]);
                }
                Physical::JoinLocal => attributes.extend([
                    attr!(u64 "hash_table_size_bytes",   1024..2u64 * 1024 * 1024 * 1024),
                    attr!(u64 "hash_table_entries",      100..50_000_000),
                    attr!(u64 "build_time_ns",           100_000..2_000_000_000),
                    attr!(u64 "probe_time_ns",           100_000..3_000_000_000),
                    attr!(u64 "build_rows",              100..10_000_000),
                    attr!(u64 "probe_rows",              100..50_000_000),
                    attr!(u64 "match_rows",              0..10_000_000),
                    attr!(f64 "hash_collision_rate",     0.0..0.3),
                    attr!(u64 "spill_count",             0..32),
                    attr!(u64 "spill_bytes",             0..4u64 * 1024 * 1024 * 1024),
                    attr!(u64 "bloom_filter_size_bytes", 0..64 * 1024 * 1024),
                    attr!(f64 "bloom_filter_fpr",        0.001..0.1),
                    // Join key columns
                    DynamicAttribute::list(
                        "join_keys",
                        DynamicList::String(
                            vec!["id", "region_id", "ts"]
                                .into_iter()
                                .take(rng().random_range(1..4))
                                .map(|s| s.to_string())
                                .collect(),
                        ),
                    ),
                    // Per-spill detail: list of structs with bytes + time
                    DynamicAttribute::list(
                        "spill_events",
                        DynamicList::Struct(
                            (0..rng().random_range(0u64..4))
                                .map(|_| {
                                    DynamicStruct(vec![
                                        DynamicAttribute::u64(
                                            "bytes",
                                            rng().random_range(1024..1024 * 1024 * 1024),
                                        ),
                                        DynamicAttribute::u64(
                                            "time_ns",
                                            rng().random_range(10_000..500_000_000),
                                        ),
                                        DynamicAttribute::u64(
                                            "rows",
                                            rng().random_range(1000..1_000_000),
                                        ),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ]),
                Physical::Sort => {
                    let num_keys: usize = rng().random_range(1..8);
                    attributes.extend([
                        attr!(pick "direction",              "asc", "desc"),
                        DynamicAttribute::u64("sort_keys", num_keys as u64),
                        attr!(u64  "comparison_count",       1000..500_000_000),
                        attr!(u64  "merge_passes",           1..16),
                        attr!(u64  "run_count",              1..512),
                        attr!(u64  "spill_count",            0..64),
                        attr!(u64  "spill_bytes",            0..4u64 * 1024 * 1024 * 1024),
                        attr!(u64  "merge_time_ns",          100_000..2_000_000_000),
                        attr!(f64  "avg_key_length_bytes",   4.0..256.0),
                        attr!(f64  "presorted_fraction",     0.0..1.0),
                        // Per sort-key specification
                        DynamicAttribute::list(
                            "key_specs",
                            DynamicList::Struct(
                                [
                                    "ts", "amount", "id", "score", "name", "region", "category",
                                    "status",
                                ]
                                .iter()
                                .take(num_keys)
                                .map(|col| {
                                    DynamicStruct(vec![
                                        DynamicAttribute::string("column", *col),
                                        DynamicAttribute::string(
                                            "direction",
                                            *rng().sample(Choose::new(&["asc", "desc"]).unwrap()),
                                        ),
                                        DynamicAttribute::string(
                                            "nulls",
                                            *rng().sample(Choose::new(&["first", "last"]).unwrap()),
                                        ),
                                    ])
                                })
                                .collect(),
                            ),
                        ),
                    ]);
                }
                Physical::Limit => attributes.extend([
                    attr!(u32 "amount",                  42),
                    attr!(u64 "rows_inspected",          42..10_000_000),
                    attr!(u64 "rows_emitted",            1..43),
                    attr!(f64 "early_termination_ratio", 0.0..1.0),
                ]),
                Physical::Output => {
                    let flush_count: u64 = rng().random_range(1..128);
                    attributes.extend([
                        attr!(pick "sink",                   "file", "memory"),
                        attr!(u64  "rows_written",           0..10_000_000),
                        attr!(u64  "bytes_written",          0..4u64 * 1024 * 1024 * 1024),
                        DynamicAttribute::u64("flush_count", flush_count),
                        attr!(u64  "flush_time_ns",          10_000..500_000_000),
                        attr!(f64  "compression_ratio",      0.1..0.9),
                        attr!(u64  "serialization_time_ns",  10_000..1_000_000_000),
                        // Per-flush durations
                        DynamicAttribute::list(
                            "per_flush_time_ns",
                            DynamicList::U64(
                                (0..flush_count)
                                    .map(|_| rng().random_range(1000..10_000_000))
                                    .collect(),
                            ),
                        ),
                    ]);
                }
            }
            physical_plan.dag[*node_idx]
                .handle
                .statistics(attributes.into())
                .unwrap();

            let edges = physical_plan
                .dag
                .edges_directed(*node_idx, Direction::Incoming)
                .map(|edge| edge.id())
                .collect::<Vec<_>>();
            for edge_idx in edges {
                let port = &mut physical_plan.dag[edge_idx].target;
                port.handle
                    .statistics(
                        vec![
                            DynamicAttribute::u64("bytes", port.num_bytes.load(Ordering::Relaxed)),
                            DynamicAttribute::u64("rows", port.num_rows.load(Ordering::Relaxed)),
                        ]
                        .into(),
                    )
                    .unwrap();
            }
        }
    }

    fn shut_down(&mut self) {
        for memory in [&mut self.filesystem, &mut self.memory] {
            memory.finalizing().unwrap();
            memory.exit().unwrap();
            sleep_long();
        }
        for channel in [&mut self.fs_to_mem, &mut self.mem_to_fs] {
            channel.finalizing().unwrap();
            channel.exit().unwrap();
            sleep_long();
        }
        for thread in &mut self.threads {
            thread.finalizing().unwrap();
            thread.exit().unwrap();
        }
        sleep_long();
        self.handle.exit().unwrap();
    }
}

struct Engine {
    handle: instr::Handle<instr::Engine>,
    workers: HashMap<Uuid, Worker>,
    network: instr::Handle<instr::Network>,
    network_links: HashMap<(Uuid, Uuid), instr::Handle<instr::NetworkChannel>>,
}

impl Engine {
    fn new(context: &SimulatorContext) -> Self {
        Self {
            handle: context.observer::<instr::Engine>().handle(),
            workers: Default::default(),
            network: context.observer::<instr::Network>().handle(),
            network_links: Default::default(),
        }
    }

    fn spawn(&mut self, context: &SimulatorContext, num_workers: usize, num_threads: usize) {
        info!("Simulating Engine:");
        info!(
            "\thttp://localhost:8080/analyzer/engine/{}",
            self.handle.uuid()
        );

        let instance_name = format!("holodeck-{:04x}", rng().random::<u32>());
        self.handle
            .init(
                instr::EngineImplementationAttributes {
                    name: Some("Simulator".into()),
                    version: Some("0.0.0-PoC".into()),
                    custom_attributes: Default::default(),
                },
                Some(instance_name),
            )
            .unwrap();

        // Workers
        let mut worker_ids = Vec::with_capacity(num_workers);
        for worker_index in 0..num_workers {
            let worker = Worker::new(
                format!("drone-{worker_index}"),
                context,
                self.handle.as_entity_ref(),
                num_threads,
            );
            let worker_id = worker.handle.uuid();
            worker_ids.push(worker_id);
            self.workers.insert(worker_id, worker);
        }

        // Engine-wide resources
        // Create a fully connected bidirectional network of workers
        self.network
            .declaration("network".to_string(), self.handle.as_entity_ref())
            .unwrap();
        let ch_obs = context.observer::<instr::NetworkChannel>();
        for worker_index in 0..worker_ids.len() {
            for other_worker_index in worker_index + 1..worker_ids.len() {
                let worker_id = worker_ids[worker_index];
                let other_worker_id = worker_ids[other_worker_index];

                let mut up_handle = ch_obs.handle();
                up_handle
                    .initializing(
                        format!("worker {worker_index} -> {other_worker_index}"),
                        self.network.as_entity_ref(),
                        self.workers.get(&worker_id).unwrap().memory.as_entity_ref(),
                        self.workers
                            .get(&other_worker_id)
                            .unwrap()
                            .memory
                            .as_entity_ref(),
                    )
                    .unwrap();
                up_handle
                    .operating(instr::NetworkChannelBounds { bytes: 0 })
                    .unwrap();
                self.network_links
                    .insert((worker_id, other_worker_id), up_handle);

                let mut down_handle = ch_obs.handle();
                down_handle
                    .initializing(
                        format!("worker {other_worker_index} -> {worker_index}"),
                        self.network.as_entity_ref(),
                        self.workers
                            .get(&other_worker_id)
                            .unwrap()
                            .memory
                            .as_entity_ref(),
                        self.workers.get(&worker_id).unwrap().memory.as_entity_ref(),
                    )
                    .unwrap();
                down_handle
                    .operating(instr::NetworkChannelBounds { bytes: 0 })
                    .unwrap();
                self.network_links
                    .insert((other_worker_id, worker_id), down_handle);
            }
        }
    }

    fn shut_down(&mut self) {
        // Tear down network
        for handle in self.network_links.values_mut() {
            handle.finalizing().unwrap();
            handle.exit().unwrap();
        }

        // Tear down workers
        for worker in self.workers.values_mut() {
            worker.shut_down();
        }

        self.handle.exit().unwrap();
        info!("Simulated engine shut down.")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize_tracing();

    let args = Args::parse();

    info!("Simulating with: {args:?}");

    let context = match args.exporter.into_options() {
        Some(provider) => SimulatorContext::try_new(provider)?,
        None => SimulatorContext::try_new(quent_model::Noop)?,
    };
    let mut engine = Engine::new(&context);

    engine.spawn(&context, args.num_workers, args.num_threads);

    for query_group_index in 0..args.num_query_groups {
        let query_group_obs = context.observer::<instr::QueryGroup>();
        let mut query_group = query_group_obs.handle();
        let query_group_id = query_group.uuid();

        info!("Simulating Query Group:");
        info!(
            "\thttp://localhost:8080/analyzer/engine/{}/query_group/{query_group_id}/list_queries",
            engine.handle.uuid()
        );

        query_group
            .declaration(
                format!("TPC-H (iteration {query_group_index})"),
                engine.handle.as_entity_ref(),
            )
            .unwrap();

        // "Run" the specified number of queries, sequentially for now.
        for query_index in 0..args.num_queries {
            let query_obs = context.observer::<instr::Query>();
            let mut query = query_obs.handle();
            query
                .init(format!("Q{query_index}"), query_group.as_entity_ref())
                .unwrap();
            info!("Simulating Query:");
            info!(
                "\thttp://localhost:8080/analyzer/engine/{}/query/{}",
                engine.handle.uuid(),
                query.uuid()
            );
            query.planning().unwrap();
            let mut l_plan = make_logical_plan(&context, query.as_entity_ref(), "logical".into());
            l_plan.declare(None);
            query.executing().unwrap();

            let workers: Vec<_> = engine.workers.values().collect();
            std::thread::scope(|s| {
                for worker in workers {
                    s.spawn(|| {
                        worker.execute_logical_plan(&context, &engine, &l_plan, args.num_tasks);
                    });
                }
            });

            query.exit().unwrap();
        }
    }

    engine.shut_down();

    // Each entity stream flushes only when its last observer clone is released.
    // `engine` co-owns those clones through its worker and network-link handles,
    // so it must drop together with the context to write all pending events.
    drop((engine, context));

    info!("simulation completed");
    Ok(())
}
