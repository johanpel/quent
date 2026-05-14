// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Fixed query-engine event emitter — a deterministic, mnemonic test fixture.
//!
//! Emits a byte-stable event stream that drives one tiny query through a full
//! lifecycle on two workers. Every UUID, timestamp, and payload is hard-coded
//! so two runs produce identical output. Used as a golden-file fixture for
//! integration tests and as a deterministic scenario for manual UI debugging.
//! The sibling `examples/simulator/` emits the same model surface with
//! runtime entropy.
//!
//! # What you will see in the UI
//!
//! Eight seconds, end to end. Major phase boundaries land on whole-second ticks.
//!
//! ```text
//!  0s ─ 1s    init       engine, 2 workers, per-worker memory + thread
//!                         pool + 2 threads, cross-worker channel
//!  1s ─ 2s    Init (Q)   query enters Init state
//!  2s ─ 3s    Planning   logical plan + two physical sub-plans declared
//!  3s ─ 4s    SCAN       4 ScanFilter tasks (2 per worker, parallel)
//!  4s ─ 5s    PARTIAL    4 PartialAggregate tasks (2 per worker, parallel);
//!                         worker-1's two tasks ship their output over the
//!                         channel from 4.5s onwards
//!  5s ─ 6s    FINAL      2 FinalAggregate tasks (worker-0 only)
//!  6s ─ 7s    LIMIT      2 Limit tasks (worker-0 only)
//!  7s ─ 8s    cleanup    operator + port statistics, then query exit and
//!                         resource teardown in reverse-init order
//! ```
//!
//! Output is declared but contributes no execution.
//!
//! # Task shape
//!
//! Each task occupies exactly 1s. Non-sender tasks (10 of 12):
//!
//! ```text
//! 0ms          250ms                              1000ms
//! ├ allocating ┼─────────── computing ────────────┤ exit
//! ```
//!
//! Sender tasks (TASK_6 and TASK_7, worker-1's PartialAggregate, 2 of 12):
//!
//! ```text
//! 0ms          250ms        500ms                 1000ms
//! ├ allocating ┼─ computing ┼─────── sending ─────┤ exit
//! ```
//!
//! Sender tasks use their own thread plus CHANNEL_W1_W0 (256 bytes) during
//! their sending state, shipping worker-1's partial aggregate over to
//! worker-0's FinalAggregate.
//!
//! # Topology
//!
//! ```text
//! engine
//! ├─ worker-0 (driver)
//! │   ├─ memory          (1 KiB)
//! │   └─ thread-pool
//! │       ├─ thread-0
//! │       └─ thread-1
//! ├─ worker-1 (contributor)
//! │   ├─ memory          (1 KiB)
//! │   └─ thread-pool
//! │       ├─ thread-0
//! │       └─ thread-1
//! └─ channel: worker-1's memory → worker-0's memory  (parented to engine)
//! ```
//!
//! # Query plan
//!
//! Logical (5 operators, linear):
//!
//! ```text
//! Scan → Filter → Aggregate → Limit → Output
//! ```
//!
//! Physical, split across workers (two sub-plans of the same logical plan):
//!
//! ```text
//! worker-0:  ScanFilter ─► PartialAggregate ─► FinalAggregate ─► Limit ─► Output
//! worker-1:  ScanFilter ─► PartialAggregate ─┐
//!                                             └─ channel to worker-0's FinalAggregate
//! ```
//!
//! Lowering patterns demonstrated:
//!
//! - `Scan + Filter → ScanFilter`                    (2:1, predicate pushdown)
//! - `Aggregate → PartialAggregate + FinalAggregate` (1:2, partial agg split)
//! - `Limit → Limit`, `Output → Output`              (1:1)
//!
//! # Mnemonic decoder ring
//!
//! - **UUIDs**: flat 1-based hex numbering in the trailing byte. Take the
//!   last 1–2 hex digits of any UUID seen in a log or the UI and grep this
//!   file for `00000001`, `00000002`, etc. Every UUID is a named const.
//! - **Timestamps**: virtual nanoseconds anchored at 0. Whole-second ticks
//!   are the phase boundaries listed above. Numbers are written as plain
//!   numeric literals so they grep cleanly.
//! - **Payloads**: trivial. `custom_attributes: Default::default()` almost
//!   everywhere. The single exception: operator statistics carry a
//!   `type: <kind_name>` string attribute matching the operator's declared
//!   `type_name`, so each stats event is self-identifying.
//!
//! Read on if you want details.

use clap::Parser;
use quent_attributes::Attribute;
use quent_exporter::{
    CollectorExporterOptions, ExporterOptions, MsgpackExporterOptions, NdjsonExporterOptions,
    PostcardExporterOptions,
};
use quent_model::{Ref, usage};
use quent_query_engine_model::{
    engine::{self, EngineImplementationAttributes},
    operator, plan, port, query_group, worker,
};
use quent_simulator_instrumentation::SimulatorContext;
use quent_time::TimeUnixNanoSec;
use uuid::{Uuid, uuid};

// Top-level entities
const ENGINE: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
const QUERY_GROUP: Uuid = uuid!("00000000-0000-0000-0000-000000000003");
const QUERY: Uuid = uuid!("00000000-0000-0000-0000-000000000004");

// Workers
const WORKER_0: Uuid = uuid!("00000000-0000-0000-0000-000000000002");
const WORKER_1: Uuid = uuid!("00000000-0000-0000-0000-000000000021");

// Per-worker resources
const MEMORY_W0: Uuid = uuid!("00000000-0000-0000-0000-000000000022");
const MEMORY_W1: Uuid = uuid!("00000000-0000-0000-0000-000000000023");
const THREAD_POOL_W0: Uuid = uuid!("00000000-0000-0000-0000-00000000003b");
const THREAD_POOL_W1: Uuid = uuid!("00000000-0000-0000-0000-00000000003c");
const THREAD_W0_T0: Uuid = uuid!("00000000-0000-0000-0000-000000000024");
const THREAD_W0_T1: Uuid = uuid!("00000000-0000-0000-0000-000000000025");
const THREAD_W1_T0: Uuid = uuid!("00000000-0000-0000-0000-000000000026");
const THREAD_W1_T1: Uuid = uuid!("00000000-0000-0000-0000-000000000027");

// Cross-worker channel (parented to engine, used by sender tasks)
const CHANNEL_W1_W0: Uuid = uuid!("00000000-0000-0000-0000-000000000028");

// Plans
const LOGICAL_PLAN: Uuid = uuid!("00000000-0000-0000-0000-000000000005");
const PHYSICAL_PLAN_W0: Uuid = uuid!("00000000-0000-0000-0000-000000000006");
const PHYSICAL_PLAN_W1: Uuid = uuid!("00000000-0000-0000-0000-00000000002e");

// Logical operators
const LOG_SCAN: Uuid = uuid!("00000000-0000-0000-0000-000000000007");
const LOG_FILTER: Uuid = uuid!("00000000-0000-0000-0000-000000000008");
const LOG_AGGREGATE: Uuid = uuid!("00000000-0000-0000-0000-000000000009");
const LOG_LIMIT: Uuid = uuid!("00000000-0000-0000-0000-00000000000a");
const LOG_OUTPUT: Uuid = uuid!("00000000-0000-0000-0000-00000000000b");

// Physical operators (worker-0)
const PHYS_SCAN_FILTER_W0: Uuid = uuid!("00000000-0000-0000-0000-00000000000c");
const PHYS_PARTIAL_AGG_W0: Uuid = uuid!("00000000-0000-0000-0000-00000000000d");
const PHYS_FINAL_AGG: Uuid = uuid!("00000000-0000-0000-0000-00000000000e");
const PHYS_LIMIT: Uuid = uuid!("00000000-0000-0000-0000-00000000000f");
const PHYS_OUTPUT: Uuid = uuid!("00000000-0000-0000-0000-000000000010");

// Physical operators (worker-1)
const PHYS_SCAN_FILTER_W1: Uuid = uuid!("00000000-0000-0000-0000-00000000002f");
const PHYS_PARTIAL_AGG_W1: Uuid = uuid!("00000000-0000-0000-0000-000000000030");

// Logical ports
const PORT_LOG_SCAN_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000011");
const PORT_LOG_FILTER_IN: Uuid = uuid!("00000000-0000-0000-0000-000000000012");
const PORT_LOG_FILTER_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000013");
const PORT_LOG_AGGREGATE_IN: Uuid = uuid!("00000000-0000-0000-0000-000000000014");
const PORT_LOG_AGGREGATE_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000015");
const PORT_LOG_LIMIT_IN: Uuid = uuid!("00000000-0000-0000-0000-000000000016");
const PORT_LOG_LIMIT_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000017");
const PORT_LOG_OUTPUT_IN: Uuid = uuid!("00000000-0000-0000-0000-000000000018");

// Physical ports (worker-0)
const PORT_PHYS_SCAN_FILTER_W0_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000019");
const PORT_PHYS_PARTIAL_AGG_W0_IN: Uuid = uuid!("00000000-0000-0000-0000-00000000001a");
const PORT_PHYS_PARTIAL_AGG_W0_OUT: Uuid = uuid!("00000000-0000-0000-0000-00000000001b");
const PORT_PHYS_FINAL_AGG_IN: Uuid = uuid!("00000000-0000-0000-0000-00000000001c");
const PORT_PHYS_FINAL_AGG_OUT: Uuid = uuid!("00000000-0000-0000-0000-00000000001d");
const PORT_PHYS_LIMIT_IN: Uuid = uuid!("00000000-0000-0000-0000-00000000001e");
const PORT_PHYS_LIMIT_OUT: Uuid = uuid!("00000000-0000-0000-0000-00000000001f");
const PORT_PHYS_OUTPUT_IN: Uuid = uuid!("00000000-0000-0000-0000-000000000020");

// Physical ports (worker-1)
const PORT_PHYS_SCAN_FILTER_W1_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000031");
const PORT_PHYS_PARTIAL_AGG_W1_IN: Uuid = uuid!("00000000-0000-0000-0000-000000000032");
const PORT_PHYS_PARTIAL_AGG_W1_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000033");

// Tasks
const TASK_0: Uuid = uuid!("00000000-0000-0000-0000-000000000029");
const TASK_1: Uuid = uuid!("00000000-0000-0000-0000-00000000002a");
const TASK_2: Uuid = uuid!("00000000-0000-0000-0000-00000000002b");
const TASK_3: Uuid = uuid!("00000000-0000-0000-0000-00000000002c");
const TASK_4: Uuid = uuid!("00000000-0000-0000-0000-00000000002d");
const TASK_5: Uuid = uuid!("00000000-0000-0000-0000-000000000034");
const TASK_6: Uuid = uuid!("00000000-0000-0000-0000-000000000035");
const TASK_7: Uuid = uuid!("00000000-0000-0000-0000-000000000036");
const TASK_8: Uuid = uuid!("00000000-0000-0000-0000-000000000037");
const TASK_9: Uuid = uuid!("00000000-0000-0000-0000-000000000038");
const TASK_10: Uuid = uuid!("00000000-0000-0000-0000-000000000039");
const TASK_11: Uuid = uuid!("00000000-0000-0000-0000-00000000003a");

// ts!(N, expr) arms the deterministic clock to N then runs expr. The first
// timestamp() call inside expr consumes N and the override clears. ts!(N)
// just arms the clock without running anything.
macro_rules! ts {
    ($ts:expr, $($body:tt)+) => {{
        ::quent_time::set_timestamp($ts);
        { $($body)+ }
    }};
    ($ts:expr) => { ::quent_time::set_timestamp($ts) };
}

#[derive(Parser, Debug)]
#[command(name = "quent-query-engine-fixed")]
#[command(about = "Emits a fixed query-engine telemetry stream", long_about = None)]
struct Args {
    #[arg(long, default_value = "collector")]
    exporter: String,

    #[arg(
        long,
        default_value = "http://localhost:7836",
        env = "QUENT_COLLECTOR_ADDRESS"
    )]
    collector_address: String,

    #[arg(long, default_value = "events")]
    output_dir: String,
}

// emit() reads top-to-bottom as the story of the scenario. Init creates
// every resource handle inline (the handles drive teardown later), then
// short helpers handle the bulky declarative phases.
fn emit(ctx: &SimulatorContext) {
    let engine_obs = ctx.engine_observer();
    let worker_obs = ctx.worker_observer();
    let group_obs = ctx.query_group_observer();
    let query_obs = ctx.query_observer();
    let mem_obs = ctx.memory_observer();
    let proc_obs = ctx.processor_observer();
    let tp_obs = ctx.thread_pool_observer();
    let ch_obs = ctx.channel_observer();

    // Init phase (0–1s). Events spread evenly across the second so the
    // init slot is visibly populated in the UI.
    ts!(
        0,
        engine_obs.create(ENGINE).init(engine::Init {
            instance_name: Some("test-engine".into()),
            implementation: EngineImplementationAttributes {
                name: Some("Fixed".into()),
                version: Some("0.0.0".into()),
                custom_attributes: Default::default(),
            },
        })
    );
    ts!(
        50_000_000,
        worker_obs.create(WORKER_0).init(worker::Init {
            parent_engine_id: Ref::new(ENGINE),
            instance_name: "worker-0".into(),
        })
    );
    ts!(
        100_000_000,
        worker_obs.create(WORKER_1).init(worker::Init {
            parent_engine_id: Ref::new(ENGINE),
            instance_name: "worker-1".into(),
        })
    );

    // Per-worker memory (1 KiB capacity each).
    let mut mem_w0 = ts!(
        150_000_000,
        mem_obs.initializing(MEMORY_W0, "memory", WORKER_0)
    );
    ts!(200_000_000, mem_w0.operating(Some(1024)));
    let mut mem_w1 = ts!(
        250_000_000,
        mem_obs.initializing(MEMORY_W1, "memory", WORKER_1)
    );
    ts!(300_000_000, mem_w1.operating(Some(1024)));

    // One thread pool per worker; threads parent to the pool, the pool to the worker.
    ts!(
        350_000_000,
        tp_obs.thread_pool(THREAD_POOL_W0, "thread-pool", WORKER_0)
    );
    ts!(
        400_000_000,
        tp_obs.thread_pool(THREAD_POOL_W1, "thread-pool", WORKER_1)
    );

    // Two threads per worker.
    let mut th_w0_t0 = ts!(
        450_000_000,
        proc_obs.initializing(THREAD_W0_T0, "thread-0", THREAD_POOL_W0)
    );
    ts!(500_000_000, th_w0_t0.operating());
    let mut th_w0_t1 = ts!(
        550_000_000,
        proc_obs.initializing(THREAD_W0_T1, "thread-1", THREAD_POOL_W0)
    );
    ts!(600_000_000, th_w0_t1.operating());
    let mut th_w1_t0 = ts!(
        650_000_000,
        proc_obs.initializing(THREAD_W1_T0, "thread-0", THREAD_POOL_W1)
    );
    ts!(700_000_000, th_w1_t0.operating());
    let mut th_w1_t1 = ts!(
        750_000_000,
        proc_obs.initializing(THREAD_W1_T1, "thread-1", THREAD_POOL_W1)
    );
    ts!(800_000_000, th_w1_t1.operating());

    // Channel from worker-1's memory to worker-0's memory. Parented to the
    // engine since it crosses worker boundaries. Used by TASK_6 and TASK_7
    // during their sending state.
    let mut channel = ts!(
        850_000_000,
        ch_obs.initializing(
            CHANNEL_W1_W0,
            "worker-1 → worker-0",
            ENGINE,
            MEMORY_W1,
            MEMORY_W0
        )
    );
    ts!(900_000_000, channel.operating(None));

    // Query group declaration, just before the query starts.
    ts!(
        950_000_000,
        group_obs.declaration(
            QUERY_GROUP,
            query_group::Declaration {
                engine_id: ENGINE,
                instance_name: "test-group".into(),
            },
        )
    );

    // Query Init (1–2s) and Planning (2–3s). Each state lasts exactly 1s.
    let mut query = ts!(
        1_000_000_000,
        query_obs.init(QUERY, "test-query", Ref::new(QUERY_GROUP))
    );
    ts!(2_000_000_000, query.planning());

    // Plan declarations stagger inside the Planning second:
    //   2.100s — logical plan + 5 ops + 8 ports
    //   2.200s — physical plan W0 + 5 ops + 8 ports
    //   2.300s — physical plan W1 + 2 ops + 3 ports
    declare_logical_plan(ctx);
    declare_physical_plan_w0(ctx);
    declare_physical_plan_w1(ctx);

    // Task execution (3–7s). Operators run sequentially across the four
    // task seconds; within each second, the two tasks per operator run in
    // parallel on the two threads of their worker.
    ts!(3_000_000_000, query.executing());
    execute_tasks(ctx);

    // Statistics for every operator and port (7.001s – 7.031s), each on its
    // own millisecond tick.
    emit_operator_statistics(ctx);
    emit_port_statistics(ctx);

    // Teardown (7.1–8s). Query exit first, then resources in reverse-init
    // order (channel, threads, memories), then workers, then engine.
    // engine.exit lands on exactly 8s.
    ts!(7_100_000_000, query.exit());
    ts!(7_150_000_000, channel.finalizing());
    ts!(7_200_000_000, channel.exit());
    ts!(7_250_000_000, th_w1_t1.finalizing());
    ts!(7_300_000_000, th_w1_t1.exit());
    ts!(7_350_000_000, th_w1_t0.finalizing());
    ts!(7_400_000_000, th_w1_t0.exit());
    ts!(7_450_000_000, th_w0_t1.finalizing());
    ts!(7_500_000_000, th_w0_t1.exit());
    ts!(7_550_000_000, th_w0_t0.finalizing());
    ts!(7_600_000_000, th_w0_t0.exit());
    ts!(7_650_000_000, mem_w1.finalizing());
    ts!(7_700_000_000, mem_w1.exit());
    ts!(7_750_000_000, mem_w0.finalizing());
    ts!(7_800_000_000, mem_w0.exit());
    ts!(
        7_850_000_000,
        worker_obs.create(WORKER_1).exit(worker::Exit)
    );
    ts!(
        7_900_000_000,
        worker_obs.create(WORKER_0).exit(worker::Exit)
    );
    ts!(8_000_000_000, engine_obs.create(ENGINE).exit(engine::Exit));
}

// Logical plan: Scan → Filter → Aggregate → Limit → Output.
// All emitted in a 13 µs window inside the Planning second.
fn declare_logical_plan(ctx: &SimulatorContext) {
    let plan_obs = ctx.plan_observer();
    let op_obs = ctx.operator_observer();
    let port_obs = ctx.port_observer();

    let edges = vec![
        plan::Edge {
            source: Ref::new(PORT_LOG_SCAN_OUT),
            target: Ref::new(PORT_LOG_FILTER_IN),
        },
        plan::Edge {
            source: Ref::new(PORT_LOG_FILTER_OUT),
            target: Ref::new(PORT_LOG_AGGREGATE_IN),
        },
        plan::Edge {
            source: Ref::new(PORT_LOG_AGGREGATE_OUT),
            target: Ref::new(PORT_LOG_LIMIT_IN),
        },
        plan::Edge {
            source: Ref::new(PORT_LOG_LIMIT_OUT),
            target: Ref::new(PORT_LOG_OUTPUT_IN),
        },
    ];
    ts!(
        2_100_000_000,
        plan_obs.declaration(
            LOGICAL_PLAN,
            plan::Declaration {
                instance_name: "logical".into(),
                parent: plan::PlanParent {
                    query_id: Some(Ref::new(QUERY)),
                    plan_id: None,
                },
                worker_id: None,
                edges,
            },
        )
    );

    let ops: [(TimeUnixNanoSec, Uuid, &str); 5] = [
        (2_100_001_000, LOG_SCAN, "Scan"),
        (2_100_002_000, LOG_FILTER, "Filter"),
        (2_100_003_000, LOG_AGGREGATE, "Aggregate"),
        (2_100_004_000, LOG_LIMIT, "Limit"),
        (2_100_005_000, LOG_OUTPUT, "Output"),
    ];
    for (t, id, name) in ops {
        ts!(
            t,
            op_obs.create(id).declaration(operator::Declaration {
                plan_id: Ref::new(LOGICAL_PLAN),
                parent_operator_ids: vec![],
                instance_name: name.into(),
                type_name: name.into(),
                custom_attributes: Default::default(),
            })
        );
    }

    let ports: [(TimeUnixNanoSec, Uuid, Uuid, &str); 8] = [
        (2_100_006_000, PORT_LOG_SCAN_OUT, LOG_SCAN, "out"),
        (2_100_007_000, PORT_LOG_FILTER_IN, LOG_FILTER, "in"),
        (2_100_008_000, PORT_LOG_FILTER_OUT, LOG_FILTER, "out"),
        (2_100_009_000, PORT_LOG_AGGREGATE_IN, LOG_AGGREGATE, "in"),
        (2_100_010_000, PORT_LOG_AGGREGATE_OUT, LOG_AGGREGATE, "out"),
        (2_100_011_000, PORT_LOG_LIMIT_IN, LOG_LIMIT, "in"),
        (2_100_012_000, PORT_LOG_LIMIT_OUT, LOG_LIMIT, "out"),
        (2_100_013_000, PORT_LOG_OUTPUT_IN, LOG_OUTPUT, "in"),
    ];
    for (t, id, op_id, name) in ports {
        ts!(
            t,
            port_obs.create(id).declaration(port::Declaration {
                operator_id: Ref::new(op_id),
                instance_name: name.into(),
            })
        );
    }
}

// Physical plan W0 (the driver):
//   ScanFilter_W0 → PartialAggregate_W0 → FinalAggregate → Limit → Output
// Parent plan is the logical plan; worker_id = WORKER_0. parent_operator_ids
// on each physical op points back at the logical op(s) it lowered from.
fn declare_physical_plan_w0(ctx: &SimulatorContext) {
    let plan_obs = ctx.plan_observer();
    let op_obs = ctx.operator_observer();
    let port_obs = ctx.port_observer();

    let edges = vec![
        plan::Edge {
            source: Ref::new(PORT_PHYS_SCAN_FILTER_W0_OUT),
            target: Ref::new(PORT_PHYS_PARTIAL_AGG_W0_IN),
        },
        plan::Edge {
            source: Ref::new(PORT_PHYS_PARTIAL_AGG_W0_OUT),
            target: Ref::new(PORT_PHYS_FINAL_AGG_IN),
        },
        plan::Edge {
            source: Ref::new(PORT_PHYS_FINAL_AGG_OUT),
            target: Ref::new(PORT_PHYS_LIMIT_IN),
        },
        plan::Edge {
            source: Ref::new(PORT_PHYS_LIMIT_OUT),
            target: Ref::new(PORT_PHYS_OUTPUT_IN),
        },
    ];
    ts!(
        2_200_000_000,
        plan_obs.declaration(
            PHYSICAL_PLAN_W0,
            plan::Declaration {
                instance_name: "physical (worker-0)".into(),
                parent: plan::PlanParent {
                    query_id: None,
                    plan_id: Some(Ref::new(LOGICAL_PLAN)),
                },
                worker_id: Some(Ref::new(WORKER_0)),
                edges,
            },
        )
    );

    let ops: [(TimeUnixNanoSec, Uuid, &str, &[Uuid]); 5] = [
        (
            2_200_001_000,
            PHYS_SCAN_FILTER_W0,
            "ScanFilter",
            &[LOG_SCAN, LOG_FILTER],
        ),
        (
            2_200_002_000,
            PHYS_PARTIAL_AGG_W0,
            "PartialAggregate",
            &[LOG_AGGREGATE],
        ),
        (
            2_200_003_000,
            PHYS_FINAL_AGG,
            "FinalAggregate",
            &[LOG_AGGREGATE],
        ),
        (2_200_004_000, PHYS_LIMIT, "Limit", &[LOG_LIMIT]),
        (2_200_005_000, PHYS_OUTPUT, "Output", &[LOG_OUTPUT]),
    ];
    for (t, id, name, parents) in ops {
        ts!(
            t,
            op_obs.create(id).declaration(operator::Declaration {
                plan_id: Ref::new(PHYSICAL_PLAN_W0),
                parent_operator_ids: parents.iter().map(|p| Ref::new(*p)).collect(),
                instance_name: name.into(),
                type_name: name.into(),
                custom_attributes: Default::default(),
            })
        );
    }

    let ports: [(TimeUnixNanoSec, Uuid, Uuid, &str); 8] = [
        (
            2_200_006_000,
            PORT_PHYS_SCAN_FILTER_W0_OUT,
            PHYS_SCAN_FILTER_W0,
            "out",
        ),
        (
            2_200_007_000,
            PORT_PHYS_PARTIAL_AGG_W0_IN,
            PHYS_PARTIAL_AGG_W0,
            "in",
        ),
        (
            2_200_008_000,
            PORT_PHYS_PARTIAL_AGG_W0_OUT,
            PHYS_PARTIAL_AGG_W0,
            "out",
        ),
        (2_200_009_000, PORT_PHYS_FINAL_AGG_IN, PHYS_FINAL_AGG, "in"),
        (
            2_200_010_000,
            PORT_PHYS_FINAL_AGG_OUT,
            PHYS_FINAL_AGG,
            "out",
        ),
        (2_200_011_000, PORT_PHYS_LIMIT_IN, PHYS_LIMIT, "in"),
        (2_200_012_000, PORT_PHYS_LIMIT_OUT, PHYS_LIMIT, "out"),
        (2_200_013_000, PORT_PHYS_OUTPUT_IN, PHYS_OUTPUT, "in"),
    ];
    for (t, id, op_id, name) in ports {
        ts!(
            t,
            port_obs.create(id).declaration(port::Declaration {
                operator_id: Ref::new(op_id),
                instance_name: name.into(),
            })
        );
    }
}

// Physical plan W1 (the contributor):
//   ScanFilter_W1 → PartialAggregate_W1
// Cross-worker edge to FinalAggregate is implicit; data flows via CHANNEL_W1_W0.
fn declare_physical_plan_w1(ctx: &SimulatorContext) {
    let plan_obs = ctx.plan_observer();
    let op_obs = ctx.operator_observer();
    let port_obs = ctx.port_observer();

    let edges = vec![plan::Edge {
        source: Ref::new(PORT_PHYS_SCAN_FILTER_W1_OUT),
        target: Ref::new(PORT_PHYS_PARTIAL_AGG_W1_IN),
    }];
    ts!(
        2_300_000_000,
        plan_obs.declaration(
            PHYSICAL_PLAN_W1,
            plan::Declaration {
                instance_name: "physical (worker-1)".into(),
                parent: plan::PlanParent {
                    query_id: None,
                    plan_id: Some(Ref::new(LOGICAL_PLAN)),
                },
                worker_id: Some(Ref::new(WORKER_1)),
                edges,
            },
        )
    );

    let ops: [(TimeUnixNanoSec, Uuid, &str, &[Uuid]); 2] = [
        (
            2_300_001_000,
            PHYS_SCAN_FILTER_W1,
            "ScanFilter",
            &[LOG_SCAN, LOG_FILTER],
        ),
        (
            2_300_002_000,
            PHYS_PARTIAL_AGG_W1,
            "PartialAggregate",
            &[LOG_AGGREGATE],
        ),
    ];
    for (t, id, name, parents) in ops {
        ts!(
            t,
            op_obs.create(id).declaration(operator::Declaration {
                plan_id: Ref::new(PHYSICAL_PLAN_W1),
                parent_operator_ids: parents.iter().map(|p| Ref::new(*p)).collect(),
                instance_name: name.into(),
                type_name: name.into(),
                custom_attributes: Default::default(),
            })
        );
    }

    let ports: [(TimeUnixNanoSec, Uuid, Uuid, &str); 3] = [
        (
            2_300_003_000,
            PORT_PHYS_SCAN_FILTER_W1_OUT,
            PHYS_SCAN_FILTER_W1,
            "out",
        ),
        (
            2_300_004_000,
            PORT_PHYS_PARTIAL_AGG_W1_IN,
            PHYS_PARTIAL_AGG_W1,
            "in",
        ),
        (
            2_300_005_000,
            PORT_PHYS_PARTIAL_AGG_W1_OUT,
            PHYS_PARTIAL_AGG_W1,
            "out",
        ),
    ];
    for (t, id, op_id, name) in ports {
        ts!(
            t,
            port_obs.create(id).declaration(port::Declaration {
                operator_id: Ref::new(op_id),
                instance_name: name.into(),
            })
        );
    }
}

// 12 tasks on a clean 1-second grid. Operators run sequentially (3s ScanFilter,
// 4s PartialAggregate, 5s FinalAggregate, 6s Limit); within each operator's
// second the two tasks run in parallel on the two threads of their worker.
// Each task: queueing + allocating at slot start, computing at +250ms,
// exit at slot end. Sender tasks (TASK_6/7 on PartialAggregate_W1) add a
// `sending` transition at slot+500ms.
fn execute_tasks(ctx: &SimulatorContext) {
    let task_obs = ctx.task_observer();

    #[rustfmt::skip]
    let tasks = [
        // (task, operator, t_q, t_a, t_c, t_e, thread, memory)
        // ScanFilter: 3–4s, parallel on both workers' threads.
        (TASK_0,  PHYS_SCAN_FILTER_W0, 3_000_000_000_u64, 3_000_000_000, 3_250_000_000, 4_000_000_000, THREAD_W0_T0, MEMORY_W0),
        (TASK_1,  PHYS_SCAN_FILTER_W0, 3_000_000_000,     3_000_000_000, 3_250_000_000, 4_000_000_000, THREAD_W0_T1, MEMORY_W0),
        (TASK_2,  PHYS_SCAN_FILTER_W1, 3_000_000_000,     3_000_000_000, 3_250_000_000, 4_000_000_000, THREAD_W1_T0, MEMORY_W1),
        (TASK_3,  PHYS_SCAN_FILTER_W1, 3_000_000_000,     3_000_000_000, 3_250_000_000, 4_000_000_000, THREAD_W1_T1, MEMORY_W1),
        // PartialAggregate: 4–5s, parallel on both workers' threads.
        (TASK_4,  PHYS_PARTIAL_AGG_W0, 4_000_000_000,     4_000_000_000, 4_250_000_000, 5_000_000_000, THREAD_W0_T0, MEMORY_W0),
        (TASK_5,  PHYS_PARTIAL_AGG_W0, 4_000_000_000,     4_000_000_000, 4_250_000_000, 5_000_000_000, THREAD_W0_T1, MEMORY_W0),
        (TASK_6,  PHYS_PARTIAL_AGG_W1, 4_000_000_000,     4_000_000_000, 4_250_000_000, 5_000_000_000, THREAD_W1_T0, MEMORY_W1),
        (TASK_7,  PHYS_PARTIAL_AGG_W1, 4_000_000_000,     4_000_000_000, 4_250_000_000, 5_000_000_000, THREAD_W1_T1, MEMORY_W1),
        // FinalAggregate: 5–6s, parallel on worker-0's threads.
        (TASK_8,  PHYS_FINAL_AGG,      5_000_000_000,     5_000_000_000, 5_250_000_000, 6_000_000_000, THREAD_W0_T0, MEMORY_W0),
        (TASK_9,  PHYS_FINAL_AGG,      5_000_000_000,     5_000_000_000, 5_250_000_000, 6_000_000_000, THREAD_W0_T1, MEMORY_W0),
        // Limit: 6–7s, parallel on worker-0's threads.
        (TASK_10, PHYS_LIMIT,          6_000_000_000,     6_000_000_000, 6_250_000_000, 7_000_000_000, THREAD_W0_T0, MEMORY_W0),
        (TASK_11, PHYS_LIMIT,          6_000_000_000,     6_000_000_000, 6_250_000_000, 7_000_000_000, THREAD_W0_T1, MEMORY_W0),
    ];
    for (task_id, op_id, t_q, t_a, t_c, t_e, thread, memory) in tasks {
        let mut task = ts!(t_q, task_obs.queueing(task_id, "task", op_id));
        ts!(t_a, task.allocating(Some(usage(Ref::new(thread)))));
        ts!(
            t_c,
            task.computing(
                Some(usage(Ref::new(thread))),
                Some(usage((Ref::new(memory), 256u64))),
            )
        );
        if task_id == TASK_6 || task_id == TASK_7 {
            ts!(
                t_q + 500_000_000,
                task.sending(
                    Some(usage(Ref::new(thread))),
                    Some(usage((Ref::new(CHANNEL_W1_W0), 256u64))),
                )
            );
        }
        ts!(t_e, task.exit());
    }
}

// Operator statistics — one event per operator (5 logical + 7 physical = 12),
// emitted on 1ms ticks starting at 7.001s. The `type` attribute matches the
// operator's declared type_name so each stats event is self-identifying.
fn emit_operator_statistics(ctx: &SimulatorContext) {
    let op_obs = ctx.operator_observer();

    #[rustfmt::skip]
    let op_stats: [(TimeUnixNanoSec, Uuid, &str); 12] = [
        (7_001_000_000, LOG_SCAN,             "Scan"),
        (7_002_000_000, LOG_FILTER,           "Filter"),
        (7_003_000_000, LOG_AGGREGATE,        "Aggregate"),
        (7_004_000_000, LOG_LIMIT,            "Limit"),
        (7_005_000_000, LOG_OUTPUT,           "Output"),
        (7_006_000_000, PHYS_SCAN_FILTER_W0,  "ScanFilter"),
        (7_007_000_000, PHYS_SCAN_FILTER_W1,  "ScanFilter"),
        (7_008_000_000, PHYS_PARTIAL_AGG_W0,  "PartialAggregate"),
        (7_009_000_000, PHYS_PARTIAL_AGG_W1,  "PartialAggregate"),
        (7_010_000_000, PHYS_FINAL_AGG,       "FinalAggregate"),
        (7_011_000_000, PHYS_LIMIT,           "Limit"),
        (7_012_000_000, PHYS_OUTPUT,          "Output"),
    ];
    for (t, op_id, type_name) in op_stats {
        ts!(
            t,
            op_obs.create(op_id).statistics(operator::Statistics {
                custom_attributes: vec![Attribute::string("type", type_name)].into(),
            })
        );
    }
}

// Port statistics — one event per port (8 logical + 11 physical = 19),
// emitted on 1ms ticks starting at 7.013s. Empty payload.
fn emit_port_statistics(ctx: &SimulatorContext) {
    let port_obs = ctx.port_observer();

    let port_stats: [(TimeUnixNanoSec, Uuid); 19] = [
        (7_013_000_000, PORT_LOG_SCAN_OUT),
        (7_014_000_000, PORT_LOG_FILTER_IN),
        (7_015_000_000, PORT_LOG_FILTER_OUT),
        (7_016_000_000, PORT_LOG_AGGREGATE_IN),
        (7_017_000_000, PORT_LOG_AGGREGATE_OUT),
        (7_018_000_000, PORT_LOG_LIMIT_IN),
        (7_019_000_000, PORT_LOG_LIMIT_OUT),
        (7_020_000_000, PORT_LOG_OUTPUT_IN),
        (7_021_000_000, PORT_PHYS_SCAN_FILTER_W0_OUT),
        (7_022_000_000, PORT_PHYS_SCAN_FILTER_W1_OUT),
        (7_023_000_000, PORT_PHYS_PARTIAL_AGG_W0_IN),
        (7_024_000_000, PORT_PHYS_PARTIAL_AGG_W1_IN),
        (7_025_000_000, PORT_PHYS_PARTIAL_AGG_W0_OUT),
        (7_026_000_000, PORT_PHYS_PARTIAL_AGG_W1_OUT),
        (7_027_000_000, PORT_PHYS_FINAL_AGG_IN),
        (7_028_000_000, PORT_PHYS_FINAL_AGG_OUT),
        (7_029_000_000, PORT_PHYS_LIMIT_IN),
        (7_030_000_000, PORT_PHYS_LIMIT_OUT),
        (7_031_000_000, PORT_PHYS_OUTPUT_IN),
    ];
    for (t, port_id) in port_stats {
        ts!(
            t,
            port_obs.create(port_id).statistics(port::Statistics {
                custom_attributes: Default::default(),
            })
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let exporter = match args.exporter.as_str() {
        "postcard" => Some(ExporterOptions::Postcard(PostcardExporterOptions {
            output_dir: args.output_dir.clone().into(),
        })),
        "messagepack" => Some(ExporterOptions::Msgpack(MsgpackExporterOptions {
            output_dir: args.output_dir.clone().into(),
        })),
        "ndjson" => Some(ExporterOptions::Ndjson(NdjsonExporterOptions {
            output_dir: args.output_dir.into(),
        })),
        "collector" => Some(ExporterOptions::Collector(CollectorExporterOptions {
            address: args.collector_address,
        })),
        "none" => None,
        _ => {
            return Err(format!(
                "invalid exporter '{}': must be postcard, messagepack, ndjson, collector, or none",
                args.exporter
            )
            .into());
        }
    };

    let ctx = SimulatorContext::try_new(ENGINE, exporter)?;
    emit(&ctx);
    drop(ctx);
    Ok(())
}
