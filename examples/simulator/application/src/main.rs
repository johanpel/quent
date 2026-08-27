use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Display},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use crossbeam_channel::{Receiver, Sender};

use clap::Parser;
use petgraph::{Directed, Direction, Graph, graph::NodeIndex, visit::EdgeRef};
use quent_dynamic_attributes::DynamicAttribute as Attribute;
use quent_io::clap::ExporterArgs;
use quent_model::{Ref, usage};
use quent_query_engine_model::{
    engine::{self, EngineImplementationAttributes},
    operator, plan, port, query_group, worker,
};
use quent_simulator_instrumentation::{SimulatorContext, data_batch::DataBatchHandle};
use rand::{RngExt, rng};
use tracing::info;
use uuid::Uuid;

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
    #[arg(long, default_value_t = 2)]
    num_tasks: usize,

    /// Number of workers
    #[arg(long, default_value_t = 2)]
    num_workers: usize,

    /// Number of threads per worker thread pool
    #[arg(long, default_value_t = 2)]
    num_threads: usize,

    /// Number of GPUs per worker
    #[arg(long, default_value_t = 2)]
    num_gpus: usize,

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

fn sleep_fixed(micros: u64) {
    std::thread::sleep(Duration::from_micros(micros * 4));
}

/// Atomically subtract `val` from `counter`, clamping at 0 to prevent
/// unsigned underflow wrapping to u64::MAX.
fn saturating_sub(counter: &AtomicU64, val: u64) {
    let mut current = counter.load(Ordering::Relaxed);
    loop {
        let new = current.saturating_sub(val);
        match counter.compare_exchange_weak(current, new, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(actual) => current = actual,
        }
    }
}

/// Simulated bandwidth limits — server-grade hardware.
const STORAGE_BANDWIDTH_MBPS: u64 = 28_000; // 28 GB/s (NVMe RAID array, 4x gen4 drives)
const PCIE_BANDWIDTH_MBPS: u64 = 63_000; // 63 GB/s (PCIe 5.0 x16)
const NETWORK_BANDWIDTH_MBPS: u64 = 50_000; // 50 GB/s (400 GbE / InfiniBand HDR)
const COMPUTE_BANDWIDTH_MBPS: u64 = 80_000; // 80 GB/s (memory-bound compute throughput)

/// Sleep to simulate a transfer at the given bandwidth (MB/s).
fn sleep_transfer(bytes: u64, bandwidth_mbps: u64) {
    let mib = (bytes / (1024 * 1024)).max(1);
    // microseconds = MiB * 1_000_000 / bandwidth_mbps
    let micros = 50 + mib * 1_000_000 / bandwidth_mbps;
    std::thread::sleep(Duration::from_micros(micros));
}

/// Storage I/O (NVMe RAID ~28 GB/s).
fn sleep_storage_io(bytes: u64) {
    sleep_transfer(bytes, STORAGE_BANDWIDTH_MBPS);
}

/// PCIe transfer: host↔GPU (~63 GB/s, PCIe 5.0 x16).
fn sleep_pcie(bytes: u64) {
    sleep_transfer(bytes, PCIE_BANDWIDTH_MBPS);
}

/// Network transfer (~50 GB/s, 400 GbE / InfiniBand).
fn sleep_network(bytes: u64) {
    sleep_transfer(bytes, NETWORK_BANDWIDTH_MBPS);
}

/// Compute-bound processing (~80 GB/s effective throughput).
fn sleep_compute(bytes: u64) {
    sleep_transfer(bytes, COMPUTE_BANDWIDTH_MBPS);
}

/// Storage I/O with occasional latency spikes (1% of the time, 4x slower).
fn sleep_storage_io_variable(bytes: u64) {
    if rng().random_ratio(1, 100) {
        sleep_transfer(bytes, STORAGE_BANDWIDTH_MBPS / 4);
    } else {
        sleep_storage_io(bytes);
    }
}

struct Operator<T: Debug> {
    id: Uuid,
    parents: Vec<Uuid>,
    kind: T,
    tasks_processed: AtomicU64,
    batches_in: AtomicU64,
    bytes_in: AtomicU64,
    rows_in: AtomicU64,
    batches_out: AtomicU64,
    bytes_out: AtomicU64,
    rows_out: AtomicU64,
}

impl<T> Operator<T>
where
    T: Debug,
{
    fn name(&self) -> String {
        format!("{:?}", self.kind)
    }

    fn new(kind: T, parents: Vec<Uuid>) -> Self {
        Self {
            id: Uuid::now_v7(),
            parents,
            kind,
            tasks_processed: AtomicU64::new(0),
            batches_in: AtomicU64::new(0),
            bytes_in: AtomicU64::new(0),
            rows_in: AtomicU64::new(0),
            batches_out: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            rows_out: AtomicU64::new(0),
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

#[derive(Debug)]
struct Port {
    id: Uuid,
    name: &'static str,
    num_bytes: AtomicU64,
    num_rows: AtomicU64,
}

#[derive(Debug)]
struct Edge {
    source: Port,
    target: Port,
}

impl Display for Edge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Edge {
    fn new(source: &'static str, target: &'static str) -> Edge {
        Edge {
            source: Port {
                id: Uuid::now_v7(),
                name: source,
                num_bytes: AtomicU64::new(0),
                num_rows: AtomicU64::new(0),
            },
            target: Port {
                id: Uuid::now_v7(),
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
    Aggregate,
    Filter,
    Udf,
    Sort,
    Limit,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Physical {
    FileSystemScan,
    GpuDecode,
    JoinPartition,
    JoinLocal,
    Aggregate,
    Filter,
    Udf,
    Sort,
    Limit,
    Output,
}

/// A work item dispatched by the scheduler to a pool thread.
struct WorkItem<'a> {
    operator_node: NodeIndex,
    operator: &'a Operator<Physical>,
    /// Input batches (empty for scan operators which produce their own).
    input_batches: Vec<Batch>,
    /// Senders for the operator's outgoing edges in the DAG.
    output_senders: Vec<&'a Sender<Batch>>,
    /// For JoinPartition: senders to other workers' JoinLocal inputs.
    shuffle_senders: Vec<&'a Sender<Batch>>,
    /// Task index for naming.
    task_index: u64,
    /// JoinLocal nodes that use selective (non-amplifying) join logic.
    selective_joins: &'a HashSet<NodeIndex>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum InputBehavior {
    /// Produces batches from nothing (FileSystemScan).
    Source,
    /// Processes one batch at a time.
    Streaming,
    /// Collects all input before processing (JoinLocal, Sort).
    Barrier,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum OutputBehavior {
    /// Sends output immediately during compute.
    Streaming,
    /// Buffers output, sends after task completion (Aggregate, Sort).
    Deferred,
    /// Consumes without producing (Output).
    Sink,
}

impl Physical {
    fn input_behavior(self) -> InputBehavior {
        match self {
            Physical::FileSystemScan => InputBehavior::Source,
            Physical::JoinLocal | Physical::Sort => InputBehavior::Barrier,
            _ => InputBehavior::Streaming,
        }
    }

    fn output_behavior(self) -> OutputBehavior {
        match self {
            Physical::Output => OutputBehavior::Sink,
            Physical::Aggregate | Physical::Sort => OutputBehavior::Deferred,
            _ => OutputBehavior::Streaming,
        }
    }
}

struct Plan<T>
where
    T: Debug,
{
    id: Uuid,
    name: String,
    query_id: Uuid,
    parent_plan_id: Option<Uuid>,
    dag: Graph<Operator<T>, Edge, Directed>,
    execute: bool,
}

impl<T: Debug> Plan<T> {
    pub fn declare(&self, context: &SimulatorContext, worker_id: Option<Uuid>) {
        let plan_obs = context.plan_observer();
        let operator_obs = context.operator_observer();
        let port_obs = context.port_observer();

        plan_obs.declaration(
            self.id,
            plan::Declaration {
                instance_name: self.name.clone(),
                parent: match self.parent_plan_id {
                    Some(parent_id) => plan::PlanParent {
                        query_id: None,
                        plan_id: Some(Ref::new(parent_id)),
                    },
                    None => plan::PlanParent {
                        query_id: Some(Ref::new(self.query_id)),
                        plan_id: None,
                    },
                },
                worker_id: worker_id.map(Ref::new),
                edges: self
                    .dag
                    .edge_references()
                    .map(|edge| plan::Edge {
                        source: Ref::new(edge.weight().source.id),
                        target: Ref::new(edge.weight().target.id),
                    })
                    .collect(),
            },
        );

        // Declare all operators
        for node_idx in self.dag.node_indices() {
            let op = &self.dag[node_idx];
            let handle = operator_obs.create(op.id);
            handle.declaration(operator::Declaration {
                plan_id: Ref::new(self.id),
                parent_operator_ids: op.parents.iter().copied().map(Ref::new).collect(),
                instance_name: format!("{}:{}", node_idx.index(), op.name()),
                type_name: op.name(),
                custom_attributes: Default::default(),
            });

            // Declare operator ports
            for (id, event) in self
                .dag
                .edges_directed(node_idx, petgraph::Direction::Incoming)
                .map(|edge| {
                    (
                        edge.weight().target.id,
                        port::Declaration {
                            operator_id: Ref::new(op.id),
                            instance_name: edge.weight().target.name.to_string(),
                        },
                    )
                })
                .chain(
                    self.dag
                        .edges_directed(node_idx, petgraph::Direction::Outgoing)
                        .map(|edge| {
                            (
                                edge.weight().source.id,
                                port::Declaration {
                                    operator_id: Ref::new(op.id),
                                    instance_name: edge.weight().source.name.to_string(),
                                },
                            )
                        }),
                )
            {
                port_obs.create(id).declaration(event)
            }
        }
    }
}

// Create the following logical plan:
//
// Scan -> Project \                        Scan -> Project \
//                  -> Join -> Aggregate                    -> Join -> Aggregate \
// Scan -> Project /                        Scan -> Project /                    \
//                                                                                -> Join -> Aggregate -> Filter -> Udf \
//                                          Scan -> Project \                    /                                       \
//                                                          -> Join -> Aggregate                                         -> Join -> Filter -> Udf \
//                                          Scan -> Project /                                                                                     \
//                                                                                                                  Scan -> Project \               \
//                                                                                                                                   -> Join ---------> Join -> Filter -> Udf -> Aggregate -> Sort -> Limit -> Output
//                                                                                                                  Scan -> Project /
//
// Each Scan -> Project lowers to: FileSystemScan -> GpuDecode
fn make_logical_plan(query_id: Uuid, name: String) -> Plan<Logical> {
    fn add_scan_project_branch(plan: &mut Graph<Operator<Logical>, Edge, Directed>) -> NodeIndex {
        let scan = plan.add_node(Operator::new(Logical::Scan, vec![]));
        let project = plan.add_node(Operator::new(Logical::Project, vec![]));
        plan.add_edge(scan, project, Edge::new("out", "in"));
        project
    }

    fn add_join(
        plan: &mut Graph<Operator<Logical>, Edge, Directed>,
        left: NodeIndex,
        right: NodeIndex,
    ) -> NodeIndex {
        let join = plan.add_node(Operator::new(Logical::Join, vec![]));
        plan.add_edge(left, join, Edge::new("out", "left"));
        plan.add_edge(right, join, Edge::new("out", "right"));
        join
    }

    let mut dag = Graph::new();

    // Left branch: join scans A and B, then pre-aggregate
    let project_a = add_scan_project_branch(&mut dag);
    let project_b = add_scan_project_branch(&mut dag);
    let join_left = add_join(&mut dag, project_a, project_b);
    let agg_left = dag.add_node(Operator::new(Logical::Aggregate, vec![]));
    dag.add_edge(join_left, agg_left, Edge::new("out", "in"));

    // Right branch: join scans C and D, then pre-aggregate
    let project_c = add_scan_project_branch(&mut dag);
    let project_d = add_scan_project_branch(&mut dag);
    let join_right = add_join(&mut dag, project_c, project_d);
    let agg_right = dag.add_node(Operator::new(Logical::Aggregate, vec![]));
    dag.add_edge(join_right, agg_right, Edge::new("out", "in"));

    // Third branch: join scans E2 and F, then pre-aggregate
    let project_e2 = add_scan_project_branch(&mut dag);
    let project_f = add_scan_project_branch(&mut dag);
    let join_third = add_join(&mut dag, project_e2, project_f);
    let agg_third = dag.add_node(Operator::new(Logical::Aggregate, vec![]));
    dag.add_edge(join_third, agg_third, Edge::new("out", "in"));

    // Combine left+right, then join with third branch
    let join_lr = add_join(&mut dag, agg_left, agg_right);
    let agg_lr = dag.add_node(Operator::new(Logical::Aggregate, vec![]));
    dag.add_edge(join_lr, agg_lr, Edge::new("out", "in"));

    let filter_lr = dag.add_node(Operator::new(Logical::Filter, vec![]));
    dag.add_edge(agg_lr, filter_lr, Edge::new("out", "in"));

    let udf_lr = dag.add_node(Operator::new(Logical::Udf, vec![]));
    dag.add_edge(filter_lr, udf_lr, Edge::new("out", "in"));

    let join_all = add_join(&mut dag, udf_lr, agg_third);

    // Mid-stage dimension lookup with concurrent scan branch
    let project_g = add_scan_project_branch(&mut dag);
    let project_h = add_scan_project_branch(&mut dag);
    let join_dim = add_join(&mut dag, project_g, project_h);

    // Late-stage join: combine main pipeline with dimension lookup
    let join_lookup = add_join(&mut dag, join_all, join_dim);

    // Post-join processing before final sort
    let post_filter = dag.add_node(Operator::new(Logical::Filter, vec![]));
    dag.add_edge(join_lookup, post_filter, Edge::new("out", "in"));

    let post_udf = dag.add_node(Operator::new(Logical::Udf, vec![]));
    dag.add_edge(post_filter, post_udf, Edge::new("out", "in"));

    let post_aggregate = dag.add_node(Operator::new(Logical::Aggregate, vec![]));
    dag.add_edge(post_udf, post_aggregate, Edge::new("out", "in"));

    let sort = dag.add_node(Operator::new(Logical::Sort, vec![]));
    dag.add_edge(post_aggregate, sort, Edge::new("out", "in"));

    let limit = dag.add_node(Operator::new(Logical::Limit, vec![]));
    dag.add_edge(sort, limit, Edge::new("out", "in"));

    let output = dag.add_node(Operator::new(Logical::Output, vec![]));
    dag.add_edge(limit, output, Edge::new("out", "in"));

    Plan {
        id: Uuid::now_v7(),
        name,
        query_id,
        parent_plan_id: None,
        dag,
        execute: false,
    }
}

fn simulate_planning(logical: &Plan<Logical>) -> Plan<Physical> {
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
        id: Uuid::now_v7(),
        name: "physical".into(),
        query_id: logical.query_id,
        parent_plan_id: Some(logical.id),
        dag: Graph::new(),
        execute: true,
    };

    lower_logical(logical, &mut physical, output, None);

    physical
}

fn lower_logical(
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
            // Scan+Project lowers to FileSystemScan → GpuDecode
            if let Some(scan_edge) = logical
                .dag
                .edges_directed(logical_current_idx, Direction::Incoming)
                .find(|edge| logical.dag[edge.source()].kind == Logical::Scan)
            {
                let scan_op = &logical.dag[scan_edge.source()];
                let scan = physical.dag.add_node(Operator::new(
                    Physical::FileSystemScan,
                    vec![current_logical_op.id, scan_op.id],
                ));
                let decode = physical.dag.add_node(Operator::new(
                    Physical::GpuDecode,
                    vec![current_logical_op.id],
                ));
                physical.dag.add_edge(scan, decode, Edge::new("out", "in"));
                if let Some((target_node, target_port)) = physical_target_idx_port {
                    physical
                        .dag
                        .add_edge(decode, target_node, Edge::new(target_port, "in"));
                }
            } else {
                unimplemented!("this shouldn't happen in this simulator, yet");
            }
        }
        Logical::Join => {
            // split up in a partition stage and join stage
            let partition = physical.dag.add_node(Operator::new(
                Physical::JoinPartition,
                vec![current_logical_op.id],
            ));
            let local = physical.dag.add_node(Operator::new(
                Physical::JoinLocal,
                vec![current_logical_op.id],
            ));
            physical
                .dag
                .add_edge(partition, local, Edge::new("build_out", "build_in"));
            physical
                .dag
                .add_edge(partition, local, Edge::new("probe_out", "probe_in"));

            if let Some((target_node, target_port)) = physical_target_idx_port {
                physical
                    .dag
                    .add_edge(local, target_node, Edge::new("out", target_port));
            }

            // Recurse up both branches
            for input_edge in logical
                .dag
                .edges_directed(logical_current_idx, Direction::Incoming)
            {
                lower_logical(
                    logical,
                    physical,
                    input_edge.source(),
                    Some((partition, input_edge.weight().target.name)),
                );
            }
        }
        Logical::Aggregate | Logical::Filter | Logical::Udf | Logical::Sort => {
            let physical_kind = match current_logical_op.kind {
                Logical::Aggregate => Physical::Aggregate,
                Logical::Filter => Physical::Filter,
                Logical::Udf => Physical::Udf,
                Logical::Sort => Physical::Sort,
                _ => unreachable!(),
            };
            let node = physical
                .dag
                .add_node(Operator::new(physical_kind, vec![current_logical_op.id]));
            if let Some((target_node, target_port)) = physical_target_idx_port {
                physical
                    .dag
                    .add_edge(node, target_node, Edge::new("out", target_port));
            }
            let input_edge = logical
                .dag
                .edges_directed(logical_current_idx, Direction::Incoming)
                .next()
                .unwrap();
            lower_logical(
                logical,
                physical,
                input_edge.source(),
                Some((node, input_edge.weight().target.name)),
            );
        }
        Logical::Limit => {
            let limit = physical
                .dag
                .add_node(Operator::new(Physical::Limit, vec![current_logical_op.id]));
            if let Some((target_node, target_port)) = physical_target_idx_port {
                physical
                    .dag
                    .add_edge(limit, target_node, Edge::new("out", target_port));
            }
            let input_edge = logical
                .dag
                .edges_directed(logical_current_idx, Direction::Incoming)
                .next()
                .unwrap();
            lower_logical(
                logical,
                physical,
                input_edge.source(),
                Some((limit, input_edge.weight().target.name)),
            );
        }
        Logical::Output => {
            let output = physical
                .dag
                .add_node(Operator::new(Physical::Output, vec![current_logical_op.id]));
            if let Some((target_node, target_port)) = physical_target_idx_port {
                physical
                    .dag
                    .add_edge(output, target_node, Edge::new("out", target_port));
            }
            let input_edge = logical
                .dag
                .edges_directed(logical_current_idx, Direction::Incoming)
                .next()
                .unwrap();
            lower_logical(
                logical,
                physical,
                input_edge.source(),
                Some((output, input_edge.weight().target.name)),
            );
        }
    }
}

/// Capacity of GPU memory per device in bytes (4 GiB).
const GPU_MEMORY_CAPACITY: u64 = 4 * 1024 * 1024 * 1024;
/// Spill GPU→host when GPU memory usage exceeds 80% of capacity.
const GPU_MEMORY_SPILL_THRESHOLD: f64 = 0.80;

#[derive(Debug)]
struct Gpu {
    id: Uuid,
    memory: Uuid,
    compute: Uuid,
    host_mem_to_gpu: Uuid,
    gpu_to_host_mem: Uuid,
    /// Tracks current GPU memory usage in bytes for spill decisions.
    memory_used: AtomicU64,
}

impl Gpu {
    fn new() -> Self {
        Self {
            id: Uuid::now_v7(),
            memory: Uuid::now_v7(),
            compute: Uuid::now_v7(),
            host_mem_to_gpu: Uuid::now_v7(),
            gpu_to_host_mem: Uuid::now_v7(),
            memory_used: AtomicU64::new(0),
        }
    }
}

#[derive(Clone)]
struct Batch {
    handle: Arc<Mutex<DataBatchHandle>>,
    bytes: u64,
    rows: u64,
    /// Index into the worker's `gpus` vec if this batch is currently on a GPU.
    /// `None` means the batch is in host memory (or in storage if `in_storage` is true).
    gpu_index: Option<usize>,
    /// Batch has been spilled to storage; memory is not tracked on host or GPU.
    in_storage: bool,
}

impl Batch {
    fn loading_to_host_memory(&self, channel: Uuid, storage: Option<Uuid>) {
        self.handle.lock().unwrap().loading_to_host_memory(
            Some(usage((Ref::new(channel), self.bytes))),
            storage.map(|id| usage((Ref::new(id), self.bytes))),
        );
    }

    fn in_host_memory(&self, memory: Uuid) {
        self.handle
            .lock()
            .unwrap()
            .in_host_memory(Some(usage((Ref::new(memory), self.bytes))));
    }

    fn loading_to_gpu_memory(&self, channel: Uuid, host_memory: Uuid) {
        self.handle.lock().unwrap().loading_to_gpu_memory(
            Some(usage((Ref::new(channel), self.bytes))),
            Some(usage((Ref::new(host_memory), self.bytes))),
        );
    }

    fn in_gpu_memory(&self, memory: Uuid) {
        self.handle
            .lock()
            .unwrap()
            .in_gpu_memory(Some(usage((Ref::new(memory), self.bytes))));
    }

    fn spilling_to_host_memory(&self, channel: Uuid, gpu_memory: Uuid) {
        self.handle.lock().unwrap().spilling_to_host_memory(
            Some(usage((Ref::new(channel), self.bytes))),
            Some(usage((Ref::new(gpu_memory), self.bytes))),
        );
    }

    fn spilling_to_storage(&self, channel: Uuid, host_memory: Uuid) {
        self.handle.lock().unwrap().spilling_to_storage(
            Some(usage((Ref::new(channel), self.bytes))),
            Some(usage((Ref::new(host_memory), self.bytes))),
        );
    }

    fn in_storage(&self, storage: Uuid) {
        self.handle
            .lock()
            .unwrap()
            .in_storage(Some(usage((Ref::new(storage), self.bytes))));
    }

    fn exit(&self) {
        self.handle.lock().unwrap().exit();
    }
}

/// Capacity of host memory per worker in bytes (16 GiB).
const HOST_MEMORY_CAPACITY: u64 = 16 * 1024 * 1024 * 1024;
/// Spill threshold: spill when host memory usage exceeds 75% of capacity.
const HOST_MEMORY_SPILL_THRESHOLD: f64 = 0.75;

struct Worker {
    id: Uuid,
    name: String,
    host_group: Uuid,
    host_memory: Uuid,
    /// Tracks current host memory usage in bytes for spill decisions.
    host_memory_used: AtomicU64,
    thread_pool: Uuid,
    storage_group: Uuid,
    storage: Uuid,
    storage_to_host: Uuid,
    host_to_storage: Uuid,
    threads: Vec<Uuid>,
    gpus: Vec<Gpu>,
    memory_handles: Vec<quent_simulator_instrumentation::memory::MemoryHandle>,
    channel_handles: Vec<quent_simulator_instrumentation::channel::ChannelHandle>,
    processor_handles: Vec<quent_simulator_instrumentation::processor::ProcessorHandle>,
}

struct PlanExecution<'a> {
    num_tasks: usize,
    log_progress: bool,
    shuffle_counters: &'a [AtomicUsize],
    num_workers: usize,
    worker_index: usize,
    shuffle_channels: &'a [Vec<(Sender<Batch>, Receiver<Batch>)>],
}

impl Worker {
    fn new(id: Uuid, name: String, num_threads: usize, num_gpus: usize) -> Self {
        Self {
            id,
            name,
            host_group: Uuid::now_v7(),
            host_memory: Uuid::now_v7(),
            host_memory_used: AtomicU64::new(0),
            thread_pool: Uuid::now_v7(),
            storage_group: Uuid::now_v7(),
            storage: Uuid::now_v7(),
            storage_to_host: Uuid::now_v7(),
            host_to_storage: Uuid::now_v7(),
            threads: std::iter::repeat_with(Uuid::now_v7)
                .take(num_threads)
                .collect(),
            gpus: std::iter::repeat_with(Gpu::new).take(num_gpus).collect(),
            memory_handles: vec![],
            channel_handles: vec![],
            processor_handles: vec![],
        }
    }

    fn spawn(&mut self, context: &SimulatorContext, parent_engine_id: Uuid) {
        let worker_obs = context.worker_observer();
        worker_obs.create(self.id).init(worker::Init {
            parent_engine_id: Ref::new(parent_engine_id),
            instance_name: self.name.clone(),
        });

        let memory_obs = context.memory_observer();
        let channel_obs = context.channel_observer();
        let processor_obs = context.processor_observer();

        // Host group: host memory + thread pool
        context
            .host_observer()
            .host(self.host_group, "Host", self.id);
        let mut host_memory =
            memory_obs.initializing(self.host_memory, "Host Memory", self.host_group);
        host_memory.operating(Some(HOST_MEMORY_CAPACITY));
        self.memory_handles.push(host_memory);

        context.thread_pool_observer().thread_pool(
            self.thread_pool,
            "Thread Pool",
            self.host_group,
        );

        // Storage group: storage + IO channels
        context
            .storage_observer()
            .storage(self.storage_group, "Storage", self.id);
        let mut storage = memory_obs.initializing(self.storage, "Storage", self.storage_group);
        storage.operating(None);
        self.memory_handles.push(storage);

        let mut storage_to_host = channel_obs.initializing(
            self.storage_to_host,
            "S2H",
            self.storage_group,
            self.storage,
            self.host_memory,
        );
        storage_to_host.operating(None);
        self.channel_handles.push(storage_to_host);

        let mut host_to_storage = channel_obs.initializing(
            self.host_to_storage,
            "H2S",
            self.storage_group,
            self.host_memory,
            self.storage,
        );
        host_to_storage.operating(None);
        self.channel_handles.push(host_to_storage);
        for (index, thread_id) in self.threads.iter().enumerate() {
            let mut thread = processor_obs.initializing(
                *thread_id,
                &format!("Thread {index}"),
                self.thread_pool,
            );
            thread.operating();
            self.processor_handles.push(thread);
        }

        // GPUs
        for (index, gpu) in self.gpus.iter().enumerate() {
            context
                .gpu_observer()
                .gpu(gpu.id, &format!("GPU {index}"), self.id);
            let mut memory =
                memory_obs.initializing(gpu.memory, &format!("GPU {index} Memory"), gpu.id);
            memory.operating(Some(GPU_MEMORY_CAPACITY));
            self.memory_handles.push(memory);

            let mut compute =
                processor_obs.initializing(gpu.compute, &format!("GPU {index} Compute"), gpu.id);
            compute.operating();
            self.processor_handles.push(compute);
        }

        // Per-GPU H2D/D2H channels live under each GPU's resource group.
        for (index, gpu) in self.gpus.iter().enumerate() {
            let mut host_to_gpu = channel_obs.initializing(
                gpu.host_mem_to_gpu,
                &format!("H2D GPU {index}"),
                gpu.id,
                self.host_memory,
                gpu.memory,
            );
            host_to_gpu.operating(None);
            self.channel_handles.push(host_to_gpu);

            let mut gpu_to_host = channel_obs.initializing(
                gpu.gpu_to_host_mem,
                &format!("D2H GPU {index}"),
                gpu.id,
                gpu.memory,
                self.host_memory,
            );
            gpu_to_host.operating(None);
            self.channel_handles.push(gpu_to_host);
        }
    }

    /// Process a single work item dispatched by the scheduler.
    /// Returns output batches that barrier operators deferred until after
    /// the task completes, so the caller can send them after marking the
    /// operator as completed.
    fn process_work_item(
        &self,
        context: &SimulatorContext,
        engine: &Engine,
        work: &WorkItem,
        thread: Uuid,
    ) -> Vec<Batch> {
        let obs = context.task_observer();
        let batch_obs = context.data_batch_observer();
        let operator = work.operator;

        let task_id = Uuid::now_v7();
        let mut task = obs.queueing(task_id, &format!("task-{}", work.task_index), operator.id);
        sleep_fixed(50);

        // FileSystemScan: create a batch from storage (heavy I/O).
        // Each scan has a different size distribution derived from its
        // operator ID to simulate data skew across input tables.
        let mut input_batches = if operator.kind == Physical::FileSystemScan {
            let batch_id = Uuid::now_v7();
            let skew = (operator.id.as_bytes()[0] % 10) as u64 + 1; // 1-10x scale
            let base_bytes = rng().random_range(32..128) * 1024 * 1024;
            let batch_bytes = base_bytes * skew;
            let batch_rows = rng().random_range(8192..65536) * skew;
            let handle = Arc::new(Mutex::new(batch_obs.initialized(batch_id, "", operator.id)));
            let batch = Batch {
                handle,
                bytes: batch_bytes,
                rows: batch_rows,
                gpu_index: None,
                in_storage: false,
            };
            // Read external files into host memory (no storage resource
            // usage — input files are externally managed).
            batch.loading_to_host_memory(self.storage_to_host, None);
            sleep_storage_io(batch_bytes);
            self.host_memory_used
                .fetch_add(batch_bytes, Ordering::Relaxed);
            batch.in_host_memory(self.host_memory);
            vec![batch]
        } else {
            work.input_batches.clone()
        };

        // Derive task resource usage from input batch size.
        // For barrier operators, use average batch size for working memory
        // since they stream through batches rather than holding all at once.
        let total_batch_bytes: u64 = input_batches.iter().map(|b| b.bytes).sum();
        let batch_bytes = if input_batches.len() > 1 {
            total_batch_bytes / input_batches.len() as u64
        } else {
            total_batch_bytes
        };
        // Working memory scales with operator complexity.
        // JoinLocal needs hash table + build/probe buffers (3-6x).
        // Sort needs merge buffers (2-4x). Others 1-2x.
        let mem_multiplier = match operator.kind {
            Physical::JoinLocal => rng().random_range(3..7),
            Physical::Aggregate => rng().random_range(2..5),
            Physical::Sort => rng().random_range(2..5),
            _ => rng().random_range(1..3),
        };
        let working_memory_bytes = batch_bytes * mem_multiplier;

        // Determine operator behavior based on kind.
        let use_gpu = operator.kind != Physical::FileSystemScan && !self.gpus.is_empty();

        // Track working memory on the appropriate resource.
        // GPU operators use GPU memory for their scratch space; CPU-only
        // operators use host memory.
        if !use_gpu {
            self.host_memory_used
                .fetch_add(working_memory_bytes, Ordering::Relaxed);
        }

        // Spill when the memory tier holding the batches exceeds its threshold.
        // Only spill batches that are actually in the pressured tier.
        let host_pressure =
            self.host_memory_used.load(Ordering::Relaxed) as f64 / HOST_MEMORY_CAPACITY as f64;
        let spill_host = host_pressure > HOST_MEMORY_SPILL_THRESHOLD;

        task.allocating(Some(usage(Ref::new(thread))));
        sleep_fixed(2);

        // Spill host-resident batches to storage under host memory pressure.
        if spill_host {
            let host_batches: Vec<usize> = input_batches
                .iter()
                .enumerate()
                .filter(|(_, b)| b.gpu_index.is_none() && !b.in_storage)
                .map(|(i, _)| i)
                .collect();
            if !host_batches.is_empty() {
                task.spilling(Some(usage(Ref::new(thread))));
                for &i in &host_batches {
                    let batch = &mut input_batches[i];
                    saturating_sub(&self.host_memory_used, batch.bytes);
                    batch.in_storage = true;
                    batch.spilling_to_storage(self.host_to_storage, self.host_memory);
                    sleep_storage_io(batch.bytes);
                    batch.in_storage(self.storage);
                }
                sleep_storage_io_variable(batch_bytes);
            }
        }

        // Loading (scan already loaded above; this is for materialization).
        if operator.kind != Physical::FileSystemScan && rng().random_bool(0.2) {
            task.loading(
                Some(usage(Ref::new(thread))),
                Some(usage((Ref::new(self.host_memory), working_memory_bytes))),
            );
            sleep_storage_io_variable(batch_bytes);
        }

        // Pick GPU if applicable. Prefer the GPU the first batch is already on.
        let gpu_index = if use_gpu {
            input_batches
                .first()
                .and_then(|b| b.gpu_index)
                .unwrap_or_else(|| rng().random_range(0..self.gpus.len()))
                .into()
        } else {
            None
        };
        let gpu = gpu_index.map(|i: usize| &self.gpus[i]);

        // Compute time scales with operator complexity and GPU availability.
        let compute_multiplier: u64 = 1;
        let gpu_multiplier: u64 = if gpu.is_some() {
            match operator.kind {
                Physical::JoinLocal => 8,     // GPU hash join kernels
                Physical::Sort => 6,          // GPU merge sort kernels
                Physical::Udf => 5,           // GPU UDF execution
                Physical::GpuDecode => 4,     // GPU decompression
                Physical::Aggregate => 4,     // GPU aggregation kernels
                Physical::JoinPartition => 3, // GPU hashing
                Physical::Filter => 0,        // GPU predicate eval (trivial bitmask)
                _ => 1,
            }
        } else {
            0
        };
        let multiplier = compute_multiplier + gpu_multiplier;
        // GPU working memory for scratch buffers, intermediate results, etc.
        let gpu_working_memory_bytes = gpu.map_or(0, |_| working_memory_bytes);
        let consumes_input = !matches!(
            operator.kind,
            Physical::FileSystemScan | Physical::GpuDecode | Physical::Output
        );

        let is_barrier = operator.kind.input_behavior() == InputBehavior::Barrier;

        if let Some(gpu) = gpu
            && is_barrier
        {
            // Barrier operators stream through input in GPU-memory-sized chunks.
            // Reserve working memory for the entire barrier operation.
            gpu.memory_used
                .fetch_add(gpu_working_memory_bytes, Ordering::Relaxed);

            let mut chunk_start = 0;
            while chunk_start < input_batches.len() {
                // Recalculate budget each iteration — GPU memory frees up as
                // consumed/spilled batches are released between chunks.
                let gpu_budget =
                    GPU_MEMORY_CAPACITY.saturating_sub(gpu.memory_used.load(Ordering::Relaxed));

                // Fill a chunk that fits within the GPU memory budget.
                let mut chunk_bytes: u64 = 0;
                let mut chunk_end = chunk_start;
                while chunk_end < input_batches.len() {
                    let next_bytes = input_batches[chunk_end].bytes;
                    if chunk_end > chunk_start && chunk_bytes + next_bytes > gpu_budget {
                        break; // Would exceed budget; stop (but always take at least one).
                    }
                    chunk_bytes += next_bytes;
                    chunk_end += 1;
                }

                // --- Loading phase: move chunk batches to GPU ---
                task.loading(
                    Some(usage(Ref::new(thread))),
                    Some(usage((Ref::new(self.host_memory), 0))),
                );
                for batch in &mut input_batches[chunk_start..chunk_end] {
                    if batch.gpu_index.is_some() {
                        continue; // Already on GPU.
                    }
                    // Reload from storage if needed.
                    if batch.in_storage {
                        batch.loading_to_host_memory(self.storage_to_host, Some(self.storage));
                        sleep_storage_io(batch.bytes);
                        self.host_memory_used
                            .fetch_add(batch.bytes, Ordering::Relaxed);
                        batch.in_host_memory(self.host_memory);
                        batch.in_storage = false;
                    }
                    // Transfer host → GPU.
                    batch.loading_to_gpu_memory(gpu.host_mem_to_gpu, self.host_memory);
                    sleep_pcie(batch.bytes);
                    saturating_sub(&self.host_memory_used, batch.bytes);
                    gpu.memory_used.fetch_add(batch.bytes, Ordering::Relaxed);
                    batch.in_gpu_memory(gpu.memory);
                    batch.gpu_index = gpu_index;
                }

                // --- Computing phase: process the chunk ---
                task.computing(
                    "",
                    chunk_bytes,
                    Some(usage(Ref::new(thread))),
                    Some(usage((Ref::new(self.host_memory), 0))),
                    Some(usage(Ref::new(gpu.compute))),
                    Some(usage((Ref::new(gpu.memory), gpu_working_memory_bytes))),
                );
                if consumes_input {
                    for batch in &input_batches[chunk_start..chunk_end] {
                        sleep_compute(batch.bytes * multiplier);
                        if let Some(gi) = batch.gpu_index {
                            saturating_sub(&self.gpus[gi].memory_used, batch.bytes);
                        }
                        batch.exit();
                    }
                } else {
                    sleep_compute(chunk_bytes * multiplier);

                    // --- Spilling phase: evict chunk batches from GPU ---
                    task.spilling(Some(usage(Ref::new(thread))));
                    for batch in &mut input_batches[chunk_start..chunk_end] {
                        if batch.gpu_index.is_none() {
                            continue;
                        }
                        saturating_sub(&gpu.memory_used, batch.bytes);
                        self.host_memory_used
                            .fetch_add(batch.bytes, Ordering::Relaxed);
                        batch.spilling_to_host_memory(gpu.gpu_to_host_mem, gpu.memory);
                        sleep_pcie(batch.bytes);
                        batch.in_host_memory(self.host_memory);
                        batch.gpu_index = None;
                    }
                }

                chunk_start = chunk_end;
            }

            // Release GPU working memory after all chunks processed.
            saturating_sub(&gpu.memory_used, gpu_working_memory_bytes);
        } else {
            // Non-barrier (streaming/pipeline) operators: load all batches to GPU, then compute.
            if let Some(gpu) = gpu {
                for batch in &mut input_batches {
                    if batch.gpu_index.is_none() {
                        // Reload from storage if needed.
                        if batch.in_storage {
                            batch.loading_to_host_memory(self.storage_to_host, Some(self.storage));
                            sleep_storage_io(batch.bytes);
                            self.host_memory_used
                                .fetch_add(batch.bytes, Ordering::Relaxed);
                            batch.in_host_memory(self.host_memory);
                            batch.in_storage = false;
                        }
                        // Transfer host → GPU.
                        batch.loading_to_gpu_memory(gpu.host_mem_to_gpu, self.host_memory);
                        sleep_pcie(batch.bytes);
                        saturating_sub(&self.host_memory_used, batch.bytes);
                        gpu.memory_used.fetch_add(batch.bytes, Ordering::Relaxed);
                        batch.in_gpu_memory(gpu.memory);
                        batch.gpu_index = gpu_index;
                    }
                }
            }

            // Track GPU working memory pressure during compute.
            if let Some(gpu) = gpu {
                gpu.memory_used
                    .fetch_add(gpu_working_memory_bytes, Ordering::Relaxed);
            }
            task.computing(
                "",
                total_batch_bytes,
                Some(usage(Ref::new(thread))),
                Some(usage((
                    Ref::new(self.host_memory),
                    if use_gpu { 0 } else { working_memory_bytes },
                ))),
                gpu.map(|gpu| usage(Ref::new(gpu.compute))),
                gpu.map(|gpu| usage((Ref::new(gpu.memory), gpu_working_memory_bytes))),
            );
            if consumes_input {
                for batch in &input_batches {
                    sleep_compute(batch.bytes * multiplier);
                    if let Some(gi) = batch.gpu_index {
                        saturating_sub(&self.gpus[gi].memory_used, batch.bytes);
                    } else if !batch.in_storage {
                        saturating_sub(&self.host_memory_used, batch.bytes);
                    }
                    batch.exit();
                }
            } else {
                sleep_compute(total_batch_bytes * multiplier);
            }
            // Release GPU working memory after compute.
            if let Some(gpu) = gpu {
                saturating_sub(&gpu.memory_used, gpu_working_memory_bytes);
            }
        }

        // Only spill GPU→host when GPU memory exceeds threshold.
        // Skip for operators that already consumed and freed their input
        // batches during the compute phase above.
        if !consumes_input && let Some(gpu) = gpu {
            for batch in &mut input_batches {
                let gpu_pressure =
                    gpu.memory_used.load(Ordering::Relaxed) as f64 / GPU_MEMORY_CAPACITY as f64;
                if gpu_pressure > GPU_MEMORY_SPILL_THRESHOLD {
                    batch.spilling_to_host_memory(gpu.gpu_to_host_mem, gpu.memory);
                    sleep_pcie(batch.bytes);
                    saturating_sub(&gpu.memory_used, batch.bytes);
                    self.host_memory_used
                        .fetch_add(batch.bytes, Ordering::Relaxed);
                    batch.in_host_memory(self.host_memory);
                    batch.gpu_index = None;

                    // Cascade: spill host→storage if host is now over threshold.
                    let hp = self.host_memory_used.load(Ordering::Relaxed) as f64
                        / HOST_MEMORY_CAPACITY as f64;
                    if hp > HOST_MEMORY_SPILL_THRESHOLD {
                        saturating_sub(&self.host_memory_used, batch.bytes);
                        batch.spilling_to_storage(self.host_to_storage, self.host_memory);
                        sleep_storage_io(batch.bytes);
                        batch.in_storage(self.storage);
                        batch.in_storage = true;
                    }
                }
                // else: batch stays on GPU for the next operator.
            }
        }

        // Release working memory from the appropriate resource.
        if !use_gpu {
            saturating_sub(&self.host_memory_used, working_memory_bytes);
        }

        // Produce output batches. Operators that defer output buffer them
        // and send after task_exit so downstream operators don't start early.
        let mut deferred_sends: Vec<Batch> = Vec::new();
        let is_deferred = operator.kind.output_behavior() == OutputBehavior::Deferred;
        match operator.kind {
            Physical::FileSystemScan | Physical::GpuDecode => {
                // Scan and decode pass through batches as-is.
                for batch in input_batches {
                    operator.batches_out.fetch_add(1, Ordering::Relaxed);
                    operator.bytes_out.fetch_add(batch.bytes, Ordering::Relaxed);
                    operator.rows_out.fetch_add(batch.rows, Ordering::Relaxed);
                    for sender in &work.output_senders {
                        let _ = sender.send(batch.clone());
                    }
                }
            }
            Physical::Output => {
                for batch in input_batches {
                    operator.batches_in.fetch_add(1, Ordering::Relaxed);
                    operator.bytes_in.fetch_add(batch.bytes, Ordering::Relaxed);
                    operator.rows_in.fetch_add(batch.rows, Ordering::Relaxed);
                    // GPU→host if needed before writing to storage.
                    if let Some(gi) = batch.gpu_index {
                        let gpu = &self.gpus[gi];
                        saturating_sub(&gpu.memory_used, batch.bytes);
                        self.host_memory_used
                            .fetch_add(batch.bytes, Ordering::Relaxed);
                        batch.spilling_to_host_memory(gpu.gpu_to_host_mem, gpu.memory);
                        sleep_pcie(batch.bytes);
                        batch.in_host_memory(self.host_memory);
                    }
                    // Write result to storage.
                    if !batch.in_storage {
                        saturating_sub(&self.host_memory_used, batch.bytes);
                    }
                    batch.spilling_to_storage(self.host_to_storage, self.host_memory);
                    sleep_storage_io(batch.bytes);
                    batch.in_storage(self.storage);
                    batch.exit();
                }
            }
            _ => {
                // Output batches go to the same GPU as the last input batch
                // (or the selected GPU for barrier operators whose input
                // batches were already evicted after chunked processing).
                let last_gpu_index: Option<usize> =
                    input_batches.last().and_then(|b| b.gpu_index).or(gpu_index);
                // Track input stats (memory already released during compute).
                for batch in &input_batches {
                    operator.batches_in.fetch_add(1, Ordering::Relaxed);
                    operator.bytes_in.fetch_add(batch.bytes, Ordering::Relaxed);
                    operator.rows_in.fetch_add(batch.rows, Ordering::Relaxed);
                }

                // Produce one output batch per input batch, with size
                // transformation applied per-batch.
                for in_batch in &input_batches {
                    let (output_bytes, output_rows) = match operator.kind {
                        Physical::JoinLocal => {
                            if work.selective_joins.contains(&work.operator_node) {
                                // Selective join: output ≈ input (dimension lookup, equi-join)
                                let keep = rng().random_range(70..100);
                                (in_batch.bytes * keep / 100, in_batch.rows * keep / 100)
                            } else {
                                // Amplifying join: many-to-many / cross join
                                let factor = rng().random_range(2..5);
                                (factor * in_batch.bytes, factor * in_batch.rows)
                            }
                        }
                        Physical::Aggregate => {
                            let denom = rng().random_range(5..15);
                            (in_batch.bytes / denom, in_batch.rows / denom.max(1))
                        }
                        Physical::Filter => {
                            let keep = rng().random_range(60..80);
                            (in_batch.bytes * keep / 100, in_batch.rows * keep / 100)
                        }
                        Physical::JoinPartition => (in_batch.bytes, in_batch.rows),
                        Physical::Udf => (in_batch.bytes, in_batch.rows),
                        Physical::Limit => {
                            let emitted_so_far = operator.rows_out.load(Ordering::Relaxed);
                            let remaining = 42u64.saturating_sub(emitted_so_far);
                            if remaining == 0 {
                                (0, 0)
                            } else {
                                let limit_rows = remaining.min(in_batch.rows);
                                let fraction = if in_batch.rows > 0 {
                                    limit_rows as f64 / in_batch.rows as f64
                                } else {
                                    1.0
                                };
                                ((in_batch.bytes as f64 * fraction) as u64, limit_rows)
                            }
                        }
                        _ => {
                            let denom = rng().random_range(1..3);
                            (in_batch.bytes / denom, in_batch.rows / denom)
                        }
                    };

                    if output_rows == 0 {
                        continue;
                    }

                    // Split large outputs into chunks so each piece
                    // can be individually spilled under memory pressure.
                    const MAX_CHUNK_BYTES: u64 = 64 * 1024 * 1024;
                    let num_chunks = (output_bytes / MAX_CHUNK_BYTES).max(1);
                    let chunk_bytes = output_bytes / num_chunks;
                    let chunk_rows = output_rows / num_chunks;

                    for _chunk in 0..num_chunks {
                        // For JoinPartition, hash-partition the output:
                        // split into N parts (one per worker) instead of
                        // broadcasting the full chunk to everyone.
                        let num_partitions = if operator.kind == Physical::JoinPartition {
                            (1 + work.shuffle_senders.len()) as u64
                        } else {
                            1
                        };
                        let part_bytes = chunk_bytes / num_partitions.max(1);
                        let part_rows = chunk_rows / num_partitions.max(1);

                        operator
                            .batches_out
                            .fetch_add(num_partitions, Ordering::Relaxed);
                        operator.bytes_out.fetch_add(chunk_bytes, Ordering::Relaxed);
                        operator.rows_out.fetch_add(chunk_rows, Ordering::Relaxed);

                        // Helper closure: create a batch, place in memory,
                        // handle spills, and return it.
                        let make_batch = |b: u64, r: u64| -> Batch {
                            let copy_id = Uuid::now_v7();
                            let mut copy = Batch {
                                handle: Arc::new(Mutex::new(batch_obs.initialized(
                                    copy_id,
                                    "",
                                    operator.id,
                                ))),
                                bytes: b,
                                rows: r,
                                gpu_index: last_gpu_index,
                                in_storage: false,
                            };

                            let mut copy_gpu_index = last_gpu_index;
                            if let Some(gi) = copy_gpu_index {
                                let gpu = &self.gpus[gi];
                                gpu.memory_used.fetch_add(b, Ordering::Relaxed);
                                copy.in_gpu_memory(gpu.memory);
                                let gpu_pressure = gpu.memory_used.load(Ordering::Relaxed) as f64
                                    / GPU_MEMORY_CAPACITY as f64;
                                if gpu_pressure > GPU_MEMORY_SPILL_THRESHOLD {
                                    saturating_sub(&gpu.memory_used, b);
                                    self.host_memory_used.fetch_add(b, Ordering::Relaxed);
                                    copy.spilling_to_host_memory(gpu.gpu_to_host_mem, gpu.memory);
                                    sleep_pcie(b);
                                    copy.in_host_memory(self.host_memory);
                                    copy_gpu_index = None;
                                }
                            } else {
                                self.host_memory_used.fetch_add(b, Ordering::Relaxed);
                                copy.in_host_memory(self.host_memory);
                            }

                            let mut copy_in_storage = false;
                            if copy_gpu_index.is_none() {
                                let hp = self.host_memory_used.load(Ordering::Relaxed) as f64
                                    / HOST_MEMORY_CAPACITY as f64;
                                if hp > HOST_MEMORY_SPILL_THRESHOLD {
                                    saturating_sub(&self.host_memory_used, b);
                                    copy.spilling_to_storage(
                                        self.host_to_storage,
                                        self.host_memory,
                                    );
                                    sleep_storage_io(b);
                                    copy.in_storage(self.storage);
                                    copy_in_storage = true;
                                }
                            }

                            copy.gpu_index = copy_gpu_index;
                            copy.in_storage = copy_in_storage;
                            copy
                        };

                        if operator.kind == Physical::JoinPartition {
                            // Local partition: send to this worker's JoinLocal.
                            let local_batch = make_batch(part_bytes, part_rows);
                            for sender in &work.output_senders {
                                let _ = sender.send(local_batch.clone());
                            }

                            // Remote partitions: one per shuffle sender with
                            // network transfer events.
                            for (i, sender) in work.shuffle_senders.iter().enumerate() {
                                let other_workers: Vec<_> =
                                    engine.workers.keys().filter(|w| **w != self.id).collect();
                                if let Some(&other) = other_workers.get(i) {
                                    let link =
                                        *engine.network_links.get(&(self.id, *other)).unwrap();
                                    task.sending(
                                        Some(usage(Ref::new(thread))),
                                        Some(usage((Ref::new(link), part_bytes))),
                                    );
                                    sleep_network(part_bytes);
                                }
                                let remote_batch = make_batch(part_bytes, part_rows);
                                let _ = sender.send(remote_batch);
                            }
                        } else {
                            // Non-JoinPartition: send full chunk to all outputs.
                            for sender in &work.output_senders {
                                let out_batch = make_batch(chunk_bytes, chunk_rows);
                                if is_deferred {
                                    deferred_sends.push(out_batch);
                                } else {
                                    let _ = sender.send(out_batch);
                                }
                            }
                        }
                    }
                }
            }
        }

        task.exit();
        operator.tasks_processed.fetch_add(1, Ordering::Relaxed);

        deferred_sends
    }

    fn execute_logical_plan(
        &self,
        context: &SimulatorContext,
        engine: &Engine,
        l_plan: &Plan<Logical>,
        execution: PlanExecution<'_>,
    ) {
        let PlanExecution {
            num_tasks,
            log_progress,
            shuffle_counters,
            num_workers,
            worker_index,
            shuffle_channels,
        } = execution;
        let physical_plan = simulate_planning(l_plan);
        physical_plan.declare(context, Some(self.id));

        let nodes = petgraph::algo::toposort(&physical_plan.dag, None).unwrap();

        if physical_plan.execute {
            let plan = &physical_plan;

            // Determine which JoinLocal nodes are selective (non-amplifying).
            // All JoinLocal nodes except one early branch join are selective.
            // The early branch join (first in topo order) acts as the
            // amplifying many-to-many join; later joins and the final
            // dimension lookup join are selective equi-joins.
            let join_local_nodes: Vec<NodeIndex> = nodes
                .iter()
                .filter(|&&n| plan.dag[n].kind == Physical::JoinLocal)
                .copied()
                .collect();
            let selective_joins: HashSet<NodeIndex> = if join_local_nodes.len() > 1 {
                // Skip the first JoinLocal in topo order (an early branch
                // join) — it becomes the amplifying cross join.
                join_local_nodes[1..].iter().copied().collect()
            } else {
                HashSet::new()
            };
            let selective_joins = &selective_joins;

            // Create a channel for each DAG edge. Batches flow from source
            // operator to target operator through these channels.
            let mut edge_channels: HashMap<
                petgraph::graph::EdgeIndex,
                (Sender<Batch>, Receiver<Batch>),
            > = HashMap::new();
            for edge_idx in plan.dag.edge_indices() {
                let (tx, rx) = crossbeam_channel::unbounded();
                edge_channels.insert(edge_idx, (tx, rx));
            }

            // Build per-operator output senders and input receivers.
            let operator_outputs: HashMap<NodeIndex, Vec<&Sender<Batch>>> = nodes
                .iter()
                .map(|&node_idx| {
                    let senders = plan
                        .dag
                        .edges_directed(node_idx, Direction::Outgoing)
                        .map(|edge| &edge_channels[&edge.id()].0)
                        .collect();
                    (node_idx, senders)
                })
                .collect();

            let operator_inputs: HashMap<NodeIndex, Vec<&Receiver<Batch>>> = nodes
                .iter()
                .map(|&node_idx| {
                    let receivers = plan
                        .dag
                        .edges_directed(node_idx, Direction::Incoming)
                        .map(|edge| &edge_channels[&edge.id()].1)
                        .collect();
                    (node_idx, receivers)
                })
                .collect();

            // Work queue: scheduler sends work items, pool threads consume.
            let (work_tx, work_rx): (Sender<WorkItem>, Receiver<WorkItem>) =
                crossbeam_channel::unbounded();

            let task_counter = AtomicU64::new(0);
            let in_flight = &AtomicU64::new(0);

            // Per-operator completion counters (shared between pool threads
            // and scheduler for barrier synchronization).
            let completed: HashMap<NodeIndex, AtomicU64> =
                nodes.iter().map(|&n| (n, AtomicU64::new(0))).collect();
            let completed = &completed;

            // Find the output (sink) node — the root of the pull chain.
            let output_node = nodes
                .iter()
                .find(|&&n| plan.dag[n].kind == Physical::Output)
                .copied()
                .expect("physical plan must have an Output operator");

            std::thread::scope(|s| {
                // Spawn pool threads that consume work items.
                for thread_id in &self.threads {
                    let work_rx = work_rx.clone();
                    let thread_id = *thread_id;
                    s.spawn(move || {
                        while let Ok(work) = work_rx.recv() {
                            let deferred =
                                self.process_work_item(context, engine, &work, thread_id);
                            completed[&work.operator_node].fetch_add(1, Ordering::Release);
                            in_flight.fetch_sub(1, Ordering::Release);
                            // Send barrier operator output after completion is
                            // recorded, so the scheduler sees the operator as
                            // done before downstream operators receive batches.
                            for batch in deferred {
                                // Local downstream channels.
                                for sender in &work.output_senders {
                                    let _ = sender.send(batch.clone());
                                }
                                // Cross-worker shuffle channels (JoinPartition
                                // sends to other workers' JoinLocal inputs).
                                for sender in &work.shuffle_senders {
                                    let _ = sender.send(batch.clone());
                                }
                            }

                            // Accumulate port stats on DAG edges using
                            // actual batch values from the operator's
                            // output counters.
                            let op = &plan.dag[work.operator_node];
                            let out_bytes = op.bytes_out.load(Ordering::Relaxed);
                            let out_rows = op.rows_out.load(Ordering::Relaxed);
                            let out_batches = op.batches_out.load(Ordering::Relaxed).max(1);
                            let avg_bytes = out_bytes / out_batches;
                            let avg_rows = out_rows / out_batches;
                            let edges = plan
                                .dag
                                .edges_directed(work.operator_node, Direction::Outgoing);
                            for edge in edges {
                                edge.weight()
                                    .source
                                    .num_bytes
                                    .fetch_add(avg_bytes, Ordering::Relaxed);
                                edge.weight()
                                    .source
                                    .num_rows
                                    .fetch_add(avg_rows, Ordering::Relaxed);
                                edge.weight()
                                    .target
                                    .num_bytes
                                    .fetch_add(avg_bytes, Ordering::Relaxed);
                                edge.weight()
                                    .target
                                    .num_rows
                                    .fetch_add(avg_rows, Ordering::Relaxed);
                            }
                        }
                    });
                }

                // Pull-based scheduler: demand flows backward from Output
                // to scans; data flows forward through the DAG.
                s.spawn(|| {
                    // Per-operator demand: how many batches this operator
                    // still needs to produce for its downstream consumer(s).
                    let mut demand: HashMap<NodeIndex, usize> =
                        nodes.iter().map(|&n| (n, 0usize)).collect();

                    // Seed demand at the output node. The output wants
                    // num_tasks batches total.
                    *demand.get_mut(&output_node).unwrap() = num_tasks;

                    // Maximum batches any single scan can produce.
                    let scan_nodes: Vec<NodeIndex> = nodes
                        .iter()
                        .filter(|&&n| plan.dag[n].kind == Physical::FileSystemScan)
                        .copied()
                        .collect();
                    let max_per_scan = num_tasks.div_ceil(scan_nodes.len().max(1));

                    // Track how many batches each operator has been
                    // dispatched to process (for termination and demand
                    // propagation).
                    let mut dispatched: HashMap<NodeIndex, usize> =
                        nodes.iter().map(|&n| (n, 0usize)).collect();

                    // Per-barrier-operator batch buffers: collect all input
                    // batches before dispatching a single work item.
                    let mut barrier_buffers: HashMap<NodeIndex, Vec<Batch>> = nodes
                        .iter()
                        .filter(|&&n| plan.dag[n].kind.input_behavior() == InputBehavior::Barrier)
                        .map(|&n| (n, Vec::new()))
                        .collect();

                    // Map each JoinPartition node to its shuffle counter
                    // index (based on position among JoinPartitions in topo
                    // order). JoinLocal nodes map to the same index as their
                    // upstream JoinPartition.
                    let mut partition_counter_idx: HashMap<NodeIndex, usize> = HashMap::new();
                    let mut counter_idx = 0;
                    for &n in &nodes {
                        if plan.dag[n].kind == Physical::JoinPartition {
                            partition_counter_idx.insert(n, counter_idx);
                            counter_idx += 1;
                        }
                    }
                    // Map JoinLocal → same counter as its upstream JoinPartition.
                    let mut local_counter_idx: HashMap<NodeIndex, usize> = HashMap::new();
                    for &n in &nodes {
                        if plan.dag[n].kind == Physical::JoinLocal {
                            for edge in plan.dag.edges_directed(n, Direction::Incoming) {
                                if let Some(&idx) = partition_counter_idx.get(&edge.source()) {
                                    local_counter_idx.insert(n, idx);
                                    break;
                                }
                            }
                        }
                    }
                    // Track which JoinPartition counters this worker has
                    // already incremented (to avoid double-counting).
                    let mut partition_incremented: HashMap<NodeIndex, bool> = HashMap::new();

                    // Build per-JoinPartition shuffle senders (to other
                    // workers) and per-JoinLocal shuffle receivers (from
                    // other workers).
                    let partition_shuffle_senders: HashMap<NodeIndex, Vec<&Sender<Batch>>> =
                        partition_counter_idx
                            .iter()
                            .map(|(&node, &idx)| {
                                let senders: Vec<&Sender<Batch>> = shuffle_channels[idx]
                                    .iter()
                                    .enumerate()
                                    .filter(|&(wi, _)| wi != worker_index)
                                    .map(|(_, (tx, _))| tx)
                                    .collect();
                                (node, senders)
                            })
                            .collect();
                    let local_shuffle_receivers: HashMap<NodeIndex, &Receiver<Batch>> =
                        local_counter_idx
                            .iter()
                            .map(|(&node, &idx)| {
                                let rx = &shuffle_channels[idx][worker_index].1;
                                (node, rx)
                            })
                            .collect();

                    // Process in reverse topological order (output first,
                    // scans last) so demand propagates backward.
                    let reverse_topo: Vec<NodeIndex> = nodes.iter().copied().rev().collect();

                    // Total batches dispatched to Output for termination.
                    let mut output_dispatched: usize = 0;

                    // Progress logging state.
                    let mut last_log_time = std::time::Instant::now();

                    loop {
                        let mut made_progress = false;

                        // Check Limit early-termination before processing
                        // any operators, to prevent demand re-propagation.
                        let limit_done = nodes.iter().any(|&n| {
                            plan.dag[n].kind == Physical::Limit
                                && plan.dag[n].rows_out.load(Ordering::Relaxed) >= 42
                        });
                        if limit_done {
                            for d in demand.values_mut() {
                                *d = 0;
                            }
                        }

                        // Auto-add demand for streaming operators that
                        // have batches waiting but no demand. This ensures
                        // all input is processed before the operator can
                        // be marked effectively done. Skip when Limit has
                        // triggered early termination.
                        if !limit_done {
                            for &node_idx in &nodes {
                                let op = &plan.dag[node_idx];
                                if op.kind.input_behavior() == InputBehavior::Streaming
                                    && demand[&node_idx] == 0
                                    && let Some(rxs) = operator_inputs.get(&node_idx)
                                {
                                    let pending: usize = rxs.iter().map(|rx| rx.len()).sum();
                                    if pending > 0 {
                                        *demand.get_mut(&node_idx).unwrap() += pending;
                                        made_progress = true;
                                    }
                                }
                            }
                        }

                        // Forward pass (topological order): compute which
                        // nodes are effectively done — all dispatched work
                        // completed and no more input will ever arrive.
                        let mut effectively_done: HashMap<NodeIndex, bool> = HashMap::new();
                        for &node_idx in &nodes {
                            let comp = completed[&node_idx].load(Ordering::Acquire) as usize;
                            let disp = dispatched[&node_idx];
                            let no_inflight = comp == disp;
                            let has_dispatched = disp > 0;
                            let upstream_done = plan
                                .dag
                                .edges_directed(node_idx, Direction::Incoming)
                                .all(|e| *effectively_done.get(&e.source()).unwrap_or(&false));
                            // Done if: dispatched at least once, all
                            // dispatched work completed, and all upstream
                            // is done (no more input will arrive).
                            // For source operators, upstream_done is
                            // vacuously true (no incoming edges).
                            let done = has_dispatched && no_inflight && upstream_done;
                            effectively_done.insert(node_idx, done);

                            // When a JoinPartition becomes effectively done,
                            // increment its shuffle counter so other workers
                            // know this worker has finished the shuffle.
                            if done
                                && partition_counter_idx.contains_key(&node_idx)
                                && !partition_incremented.contains_key(&node_idx)
                            {
                                let idx = partition_counter_idx[&node_idx];
                                shuffle_counters[idx].fetch_add(1, Ordering::Release);
                                partition_incremented.insert(node_idx, true);
                            }
                        }

                        for &node_idx in &reverse_topo {
                            let node_demand = demand[&node_idx];
                            if node_demand == 0 {
                                continue;
                            }

                            let op = &plan.dag[node_idx];
                            let outputs = &operator_outputs[&node_idx];

                            if op.kind.input_behavior() == InputBehavior::Source {
                                // Don't over-dispatch scans.
                                if dispatched[&node_idx] >= max_per_scan {
                                    *demand.get_mut(&node_idx).unwrap() = 0;
                                    continue;
                                }
                                let idx = task_counter.fetch_add(1, Ordering::Relaxed);
                                in_flight.fetch_add(1, Ordering::Acquire);
                                let _ = work_tx.send(WorkItem {
                                    operator_node: node_idx,
                                    operator: op,
                                    input_batches: vec![],
                                    output_senders: outputs.clone(),
                                    shuffle_senders: vec![],
                                    task_index: idx,
                                    selective_joins,
                                });
                                *demand.get_mut(&node_idx).unwrap() -= 1;
                                *dispatched.get_mut(&node_idx).unwrap() += 1;
                                made_progress = true;
                            } else if op.kind.input_behavior() == InputBehavior::Barrier {
                                // Barrier operator: collect batches into
                                // buffer and dispatch only when all upstream
                                // operators have completed.
                                let inputs = &operator_inputs[&node_idx];
                                for rx in inputs {
                                    while let Ok(batch) = rx.try_recv() {
                                        barrier_buffers.get_mut(&node_idx).unwrap().push(batch);
                                        made_progress = true;
                                    }
                                }

                                // Check if all upstream operators are done
                                // using the forward-pass effectively_done map.
                                let incoming: Vec<NodeIndex> = plan
                                    .dag
                                    .edges_directed(node_idx, Direction::Incoming)
                                    .map(|e| e.source())
                                    .collect();
                                let upstream_done = incoming
                                    .iter()
                                    .all(|&src| *effectively_done.get(&src).unwrap_or(&false));

                                // For JoinLocal, also wait until all workers
                                // have finished the upstream JoinPartition.
                                let shuffle_done =
                                    if let Some(&idx) = local_counter_idx.get(&node_idx) {
                                        shuffle_counters[idx].load(Ordering::Acquire) >= num_workers
                                    } else {
                                        true
                                    };

                                if upstream_done && shuffle_done {
                                    // For JoinLocal, also drain batches from
                                    // other workers' shuffle channels.
                                    if let Some(rx) = local_shuffle_receivers.get(&node_idx) {
                                        while let Ok(batch) = rx.try_recv() {
                                            barrier_buffers.get_mut(&node_idx).unwrap().push(batch);
                                        }
                                    }

                                    let buffer = barrier_buffers.get_mut(&node_idx).unwrap();
                                    if !buffer.is_empty() {
                                        let batches = std::mem::take(buffer);
                                        let shuffle_tx = partition_shuffle_senders
                                            .get(&node_idx)
                                            .cloned()
                                            .unwrap_or_default();

                                        // Streaming-output barriers (e.g. JoinLocal)
                                        // can process batches in parallel across
                                        // threads. Split the buffer into chunks.
                                        let num_tasks = if op.kind.output_behavior()
                                            == OutputBehavior::Streaming
                                        {
                                            self.threads.len().min(batches.len())
                                        } else {
                                            1
                                        };
                                        let chunk_size = batches.len().div_ceil(num_tasks.max(1));
                                        let chunks: Vec<Vec<Batch>> = batches
                                            .into_iter()
                                            .collect::<Vec<_>>()
                                            .chunks(chunk_size)
                                            .map(|c| c.to_vec())
                                            .collect();

                                        for chunk in chunks {
                                            let idx = task_counter.fetch_add(1, Ordering::Relaxed);
                                            in_flight.fetch_add(1, Ordering::Acquire);
                                            let _ = work_tx.send(WorkItem {
                                                operator_node: node_idx,
                                                operator: op,
                                                input_batches: chunk,
                                                output_senders: outputs.clone(),
                                                shuffle_senders: shuffle_tx.clone(),
                                                task_index: idx,
                                                selective_joins,
                                            });
                                            *dispatched.get_mut(&node_idx).unwrap() += 1;
                                        }
                                        *demand.get_mut(&node_idx).unwrap() = 0;
                                        made_progress = true;
                                    }
                                } else {
                                    // Propagate demand upstream.
                                    let num_sources = incoming.len().max(1);
                                    let needed_total = node_demand + dispatched[&node_idx];
                                    let per_source = needed_total.div_ceil(num_sources);
                                    for source in incoming {
                                        let already = dispatched[&source] + demand[&source];
                                        if already < per_source {
                                            *demand.get_mut(&source).unwrap() +=
                                                per_source - already;
                                        }
                                    }
                                }
                            } else {
                                // Pipeline operator: dispatch one task per batch.
                                let inputs = &operator_inputs[&node_idx];

                                // For operators with multiple inputs, only
                                // dispatch when every input has at least one
                                // batch ready OR its source is done (channel
                                // drained). This prevents one fast path
                                // (e.g. a scan) from running far ahead of a
                                // slow path (e.g. a long join pipeline).
                                let all_inputs_ready = if inputs.len() > 1 {
                                    let incoming_nodes: Vec<NodeIndex> = plan
                                        .dag
                                        .edges_directed(node_idx, Direction::Incoming)
                                        .map(|e| e.source())
                                        .collect();
                                    incoming_nodes.iter().zip(inputs.iter()).all(|(&src, rx)| {
                                        !rx.is_empty()
                                            || *effectively_done.get(&src).unwrap_or(&false)
                                    })
                                } else {
                                    true
                                };

                                let mut got_batch = false;
                                if all_inputs_ready {
                                    for rx in inputs {
                                        if let Ok(batch) = rx.try_recv() {
                                            let idx = task_counter.fetch_add(1, Ordering::Relaxed);
                                            in_flight.fetch_add(1, Ordering::Acquire);
                                            let shuffle_tx = partition_shuffle_senders
                                                .get(&node_idx)
                                                .cloned()
                                                .unwrap_or_default();
                                            let _ = work_tx.send(WorkItem {
                                                operator_node: node_idx,
                                                operator: op,
                                                input_batches: vec![batch],
                                                output_senders: outputs.clone(),
                                                shuffle_senders: shuffle_tx,
                                                task_index: idx,
                                                selective_joins,
                                            });
                                            *demand.get_mut(&node_idx).unwrap() -= 1;
                                            *dispatched.get_mut(&node_idx).unwrap() += 1;
                                            if node_idx == output_node {
                                                output_dispatched += 1;
                                            }
                                            got_batch = true;
                                            made_progress = true;
                                            break;
                                        }
                                    }
                                }

                                if !got_batch {
                                    // Propagate demand upstream.
                                    let incoming: Vec<NodeIndex> = plan
                                        .dag
                                        .edges_directed(node_idx, Direction::Incoming)
                                        .map(|e| e.source())
                                        .collect();
                                    let num_sources = incoming.len().max(1);
                                    let needed_total = node_demand + dispatched[&node_idx];
                                    let per_source = needed_total.div_ceil(num_sources);
                                    for source in incoming {
                                        let already = dispatched[&source] + demand[&source];
                                        if already < per_source {
                                            *demand.get_mut(&source).unwrap() +=
                                                per_source - already;
                                        }
                                    }
                                }
                            }
                        }

                        // Log progress every second: per-operator completed counts.
                        if log_progress && last_log_time.elapsed().as_millis() >= 1000 {
                            let mut parts: Vec<String> = Vec::new();
                            for &node_idx in &nodes {
                                let op = &plan.dag[node_idx];
                                let comp = completed[&node_idx].load(Ordering::Relaxed);
                                let disp = dispatched[&node_idx] as u64;
                                if disp > 0 && comp < disp {
                                    parts.push(format!("{:?} {}/{}", op.kind, comp, disp));
                                }
                            }
                            if !parts.is_empty() {
                                info!("  {}", parts.join(" | "));
                            }
                            last_log_time = std::time::Instant::now();
                        }

                        // Terminate when all demand is satisfied AND all
                        // dispatched work has completed.
                        let all_demand_zero = demand.values().all(|&d| d == 0);
                        let current_in_flight = in_flight.load(Ordering::Acquire);
                        let all_done = output_dispatched >= num_tasks || all_demand_zero;
                        if all_done && current_in_flight == 0 {
                            // Drain any remaining batches from channels that
                            // were produced by in-flight work after Limit
                            // terminated.
                            for node_idx in &reverse_topo {
                                let inputs = &operator_inputs[node_idx];
                                for rx in inputs {
                                    while let Ok(_batch) = rx.try_recv() {}
                                }
                            }
                            break;
                        }

                        if !made_progress {
                            std::thread::sleep(Duration::from_micros(10));
                        }
                    }

                    // Drop the work sender to signal pool threads to exit.
                    drop(work_tx);
                });
            });
        }

        let op_obs = context.operator_observer();
        let port_obs = context.port_observer();
        for node_idx in nodes.iter() {
            let op = &physical_plan.dag[*node_idx];
            let tasks_processed = op.tasks_processed.load(Ordering::Relaxed);

            let batches_in = op.batches_in.load(Ordering::Relaxed);
            let bytes_in = op.bytes_in.load(Ordering::Relaxed);
            let rows_in = op.rows_in.load(Ordering::Relaxed);
            let batches_out = op.batches_out.load(Ordering::Relaxed);
            let bytes_out = op.bytes_out.load(Ordering::Relaxed);
            let rows_out = op.rows_out.load(Ordering::Relaxed);

            // Estimate peak memory from batch throughput and operator type.
            let mem_mult: u64 = match op.kind {
                Physical::JoinLocal => 5,
                Physical::Aggregate => 4,
                Physical::GpuDecode => 3,
                Physical::Sort => 3,
                _ => 2,
            };
            let peak_memory = (bytes_in / batches_in.max(1)) * mem_mult;

            let mut attributes = vec![
                Attribute::u64("tasks_processed", tasks_processed),
                Attribute::u64("peak_memory_bytes", peak_memory),
                Attribute::u64("output_rows", rows_out),
                Attribute::u64("output_bytes", bytes_out),
                Attribute::u64("output_batches", batches_out),
                Attribute::u64("input_rows", rows_in),
                Attribute::u64("input_bytes", bytes_in),
                Attribute::u64("input_batches", batches_in),
            ];

            match op.kind {
                Physical::FileSystemScan => {
                    let selectivity: f64 = rng().random_range(0.001..1.0);
                    let bytes_read = (bytes_out as f64 / selectivity) as u64;
                    attributes.extend([
                        Attribute::u64("files_scanned", batches_out.max(1)),
                        Attribute::u64("bytes_read", bytes_read),
                        Attribute::f64("predicate_selectivity", selectivity),
                    ]);
                }
                Physical::GpuDecode => {
                    let compression_ratio: f64 = rng().random_range(2.0..8.0);
                    attributes.extend([
                        Attribute::u64("compressed_bytes", bytes_in),
                        Attribute::u64(
                            "decompressed_bytes",
                            (bytes_in as f64 * compression_ratio) as u64,
                        ),
                        Attribute::f64("compression_ratio", compression_ratio),
                    ]);
                }
                Physical::JoinPartition => {
                    let build_bytes = bytes_in / 2;
                    let probe_bytes = bytes_in - build_bytes;
                    attributes.extend([
                        Attribute::u64("build_side_bytes", build_bytes),
                        Attribute::u64("probe_side_bytes", probe_bytes),
                        Attribute::u64("network_bytes_sent", bytes_in),
                    ]);
                }
                Physical::JoinLocal => {
                    let build_rows = rows_in / 2;
                    let probe_rows = rows_in - build_rows;
                    attributes.extend([
                        Attribute::u64("hash_table_size_bytes", bytes_in / 2),
                        Attribute::u64("hash_table_entries", build_rows),
                        Attribute::u64("build_rows", build_rows),
                        Attribute::u64("probe_rows", probe_rows),
                        Attribute::u64("match_rows", rows_out),
                    ]);
                }
                Physical::Aggregate => {
                    let reduction = if rows_in > 0 {
                        rows_in as f64 / rows_out.max(1) as f64
                    } else {
                        1.0
                    };
                    attributes.extend([
                        Attribute::u64("groups_created", rows_out),
                        Attribute::f64("reduction_factor", reduction),
                    ]);
                }
                Physical::Filter => {
                    let selectivity = if rows_in > 0 {
                        rows_out as f64 / rows_in as f64
                    } else {
                        1.0
                    };
                    attributes.extend([
                        Attribute::f64("selectivity", selectivity),
                        Attribute::u64("rows_passed", rows_out),
                        Attribute::u64("rows_filtered", rows_in.saturating_sub(rows_out)),
                    ]);
                }
                Physical::Udf => {
                    attributes.extend([
                        Attribute::string("udf_name", "apply_transform"),
                        Attribute::string("udf_language", "python"),
                        Attribute::u64("rows_processed", rows_in),
                    ]);
                }
                Physical::Sort => {
                    let n = rows_in.max(1);
                    let log_n = (n as f64).log2().max(1.0) as u64;
                    attributes.extend([
                        Attribute::u64("comparison_count", n * log_n),
                        Attribute::u64("run_count", batches_in.max(1)),
                    ]);
                }
                Physical::Limit => {
                    let ratio = if rows_in > 0 {
                        1.0 - (rows_out as f64 / rows_in as f64)
                    } else {
                        0.0
                    };
                    attributes.extend([
                        Attribute::u64("amount", 42),
                        Attribute::u64("rows_inspected", rows_in),
                        Attribute::u64("rows_emitted", rows_out),
                        Attribute::f64("early_termination_ratio", ratio),
                    ]);
                }
                Physical::Output => {
                    attributes.extend([
                        Attribute::u64("rows_written", rows_in),
                        Attribute::u64("bytes_written", bytes_in),
                        Attribute::u64("flush_count", batches_in.max(1)),
                    ]);
                }
            }
            op_obs.create(op.id).statistics(operator::Statistics {
                custom_attributes: attributes.into(),
            });

            let edges = physical_plan
                .dag
                .edges_directed(*node_idx, Direction::Incoming);
            for edge in edges {
                let port = &edge.weight().target;
                port_obs.create(port.id).statistics(port::Statistics {
                    custom_attributes: vec![
                        Attribute::u64("bytes", port.num_bytes.load(Ordering::Relaxed)),
                        Attribute::u64("rows", port.num_rows.load(Ordering::Relaxed)),
                    ]
                    .into(),
                });
            }
        }
    }

    fn shut_down(&mut self, context: &SimulatorContext) {
        let worker_obs = context.worker_observer();
        for handle in &mut self.memory_handles {
            handle.finalizing();
            handle.exit();
        }
        for handle in &mut self.channel_handles {
            handle.finalizing();
            handle.exit();
        }
        for handle in &mut self.processor_handles {
            handle.finalizing();
            handle.exit();
        }
        worker_obs.create(self.id).exit(worker::Exit);
    }
}

struct Engine {
    id: Uuid,
    workers: HashMap<Uuid, Worker>,
    network: Uuid,
    network_links: HashMap<(Uuid, Uuid), Uuid>,
    network_link_handles: Vec<quent_simulator_instrumentation::channel::ChannelHandle>,
}

impl Engine {
    fn new() -> Self {
        Self {
            id: Uuid::now_v7(),
            workers: Default::default(),
            network: Uuid::now_v7(),
            network_links: Default::default(),
            network_link_handles: vec![],
        }
    }

    fn spawn(
        &mut self,
        context: &SimulatorContext,
        num_workers: usize,
        num_threads: usize,
        num_gpus: usize,
    ) {
        // Create some observers
        info!("Simulating Engine {}", self.id);
        let engine_obs = context.engine_observer();
        let engine_handle = engine_obs.create(self.id);
        engine_handle.init(engine::Init {
            instance_name: Some(format!("holodeck-{:04x}", rng().random::<u32>())),
            implementation: EngineImplementationAttributes {
                name: Some("Simulator".into()),
                version: Some("0.0.0-PoC".into()),
                custom_attributes: Default::default(),
            },
        });

        // Workers
        let worker_ids = std::iter::repeat_with(Uuid::now_v7)
            .take(num_workers)
            .collect::<Vec<_>>();

        for (worker_index, worker_id) in worker_ids.iter().enumerate() {
            let mut worker = Worker::new(
                *worker_id,
                format!("drone-{worker_index}"),
                num_threads,
                num_gpus,
            );
            worker.spawn(context, self.id);
            self.workers.insert(*worker_id, worker);
        }

        // Engine-wide resources
        // Create a fully connected bidirectional network of workers
        context
            .network_observer()
            .network(self.network, "network", self.id);
        let channel_obs = context.channel_observer();
        for worker_index in 0..worker_ids.len() {
            for other_worker_index in worker_index + 1..worker_ids.len() {
                let worker_id = worker_ids[worker_index];
                let other_worker_id = worker_ids[other_worker_index];
                let up_link_id = Uuid::now_v7();
                let mut up_link = channel_obs.initializing(
                    up_link_id,
                    &format!("worker {worker_index} -> {other_worker_index}"),
                    self.network,
                    self.workers.get(&worker_id).unwrap().host_memory,
                    self.workers.get(&other_worker_id).unwrap().host_memory,
                );
                up_link.operating(None);
                self.network_link_handles.push(up_link);

                let down_link_id = Uuid::now_v7();
                let mut down_link = channel_obs.initializing(
                    down_link_id,
                    &format!("worker {other_worker_index} -> {worker_index}"),
                    self.network,
                    self.workers.get(&other_worker_id).unwrap().host_memory,
                    self.workers.get(&worker_id).unwrap().host_memory,
                );
                down_link.operating(None);
                self.network_link_handles.push(down_link);

                self.network_links
                    .insert((worker_id, other_worker_id), up_link_id);
                self.network_links
                    .insert((other_worker_id, worker_id), down_link_id);
            }
        }
    }

    fn shut_down(&mut self, context: &SimulatorContext) {
        // Create some observers
        let engine_obs = context.engine_observer();
        for handle in &mut self.network_link_handles {
            handle.finalizing();
            handle.exit();
        }

        // Tear down workers
        for worker in self.workers.values_mut() {
            worker.shut_down(context);
        }

        engine_obs.create(self.id).exit(engine::Exit);
        info!("Simulated engine shut down.")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize_tracing();

    let args = Args::parse();

    info!("Simulating with: {args:?}");

    let mut engine = Engine::new();

    let context = match args.exporter.into_options() {
        Some(provider) => SimulatorContext::try_new(provider)?,
        None => SimulatorContext::try_new(quent_model::Noop)?,
    };

    engine.spawn(&context, args.num_workers, args.num_threads, args.num_gpus);

    for (query_group_index, query_group_id) in std::iter::repeat_with(Uuid::now_v7)
        .take(args.num_query_groups)
        .enumerate()
    {
        let query_group_obs = context.query_group_observer();
        query_group_obs.declaration(
            query_group_id,
            query_group::Declaration {
                engine_id: engine.id,
                instance_name: format!("FPC-H (run {query_group_index})"),
            },
        );

        // "Run" the specified number of queries, sequentially for now.
        for (query_index, query_id) in std::iter::repeat_with(Uuid::now_v7)
            .take(args.num_queries)
            .enumerate()
        {
            let total = args.num_query_groups * args.num_queries;
            let done = query_group_index * args.num_queries + query_index;
            info!("{}% ({}/{})", done * 100 / total, done, total);
            let query_obs = context.query_observer();
            let mut query = query_obs.init(
                query_id,
                &{
                    const QUERY_NUMBERS: &[u32] = &[42, 1337, 7, 404, 256, 99, 13, 1024, 69, 314];
                    let n = QUERY_NUMBERS[query_index % QUERY_NUMBERS.len()];
                    format!("Q{n}")
                },
                Ref::new(query_group_id),
            );
            query.planning();
            let l_plan = make_logical_plan(query_id, "logical".into());
            l_plan.declare(&context, None);
            query.executing();

            let workers: Vec<_> = engine.workers.values().collect();
            // Count JoinPartition operators in the logical plan to size
            // the shuffle counters. Each logical Join lowers to one
            // JoinPartition, so count Join nodes.
            let num_joins = l_plan
                .dag
                .node_indices()
                .filter(|&n| l_plan.dag[n].kind == Logical::Join)
                .count();
            let shuffle_counters: Vec<AtomicUsize> =
                (0..num_joins).map(|_| AtomicUsize::new(0)).collect();
            let num_workers = workers.len();
            // Create cross-worker shuffle channels:
            // shuffle_channels[join_stage][target_worker] = (Sender, Receiver)
            let shuffle_channels: Vec<Vec<(Sender<Batch>, Receiver<Batch>)>> = (0..num_joins)
                .map(|_| {
                    (0..num_workers)
                        .map(|_| crossbeam_channel::unbounded())
                        .collect()
                })
                .collect();
            std::thread::scope(|s| {
                let context = &context;
                let engine = &engine;
                let l_plan = &l_plan;
                let shuffle_counters = &shuffle_counters[..];
                let shuffle_channels = &shuffle_channels;
                let num_workers = workers.len();
                for (i, worker) in workers.iter().enumerate() {
                    let log_progress = i == 0;
                    s.spawn(move || {
                        worker.execute_logical_plan(
                            context,
                            engine,
                            l_plan,
                            PlanExecution {
                                num_tasks: args.num_tasks,
                                log_progress,
                                shuffle_counters,
                                num_workers,
                                worker_index: i,
                                shuffle_channels,
                            },
                        );
                    });
                }
            });

            query.exit();
        }
    }

    engine.shut_down(&context);

    drop((engine, context));
    info!("simulation completed");
    Ok(())
}
