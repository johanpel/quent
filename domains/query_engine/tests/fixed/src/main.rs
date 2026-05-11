// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Deterministic query-engine event emitter.
//!
//! Emits a fixed, byte-stable telemetry stream — every UUID, timestamp, and
//! payload is determined at compile time. Used for golden-file integration
//! tests and manual UI debugging. The sibling `examples/simulator/` emits
//! the same model surface with runtime entropy.

use clap::Parser;
use quent_exporter::{
    CollectorExporterOptions, ExporterOptions, MsgpackExporterOptions, NdjsonExporterOptions,
    PostcardExporterOptions,
};
use quent_model::{Ref, instrumentation, model};
use quent_query_engine_model::{
    engine::{self, EngineImplementationAttributes},
    operator, plan, port, query_group, worker,
};
use uuid::{Uuid, uuid};

model! {
    Fixed {
        root: quent_query_engine_model::engine::Engine,
        quent_query_engine_model::worker::Worker,
        quent_query_engine_model::query_group::QueryGroup,
        quent_query_engine_model::query::Query,
        quent_query_engine_model::plan::Plan,
        quent_query_engine_model::operator::Operator,
        quent_query_engine_model::port::Port,
    }
}
instrumentation!(Fixed);

// Entity UUIDs — flat sequential numbering. Look up the trailing hex byte in
// an event log against this list to identify which entity it is.
const ENGINE: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
const WORKER: Uuid = uuid!("00000000-0000-0000-0000-000000000002");
const QUERY_GROUP: Uuid = uuid!("00000000-0000-0000-0000-000000000003");
const QUERY: Uuid = uuid!("00000000-0000-0000-0000-000000000004");

const LOGICAL_PLAN: Uuid = uuid!("00000000-0000-0000-0000-000000000005");
const PHYSICAL_PLAN: Uuid = uuid!("00000000-0000-0000-0000-000000000006");

const LOG_SCAN: Uuid = uuid!("00000000-0000-0000-0000-000000000007");
const LOG_FILTER: Uuid = uuid!("00000000-0000-0000-0000-000000000008");
const LOG_AGGREGATE: Uuid = uuid!("00000000-0000-0000-0000-000000000009");
const LOG_LIMIT: Uuid = uuid!("00000000-0000-0000-0000-00000000000a");
const LOG_OUTPUT: Uuid = uuid!("00000000-0000-0000-0000-00000000000b");

const PHYS_SCAN_FILTER: Uuid = uuid!("00000000-0000-0000-0000-00000000000c");
const PHYS_PARTIAL_AGG: Uuid = uuid!("00000000-0000-0000-0000-00000000000d");
const PHYS_FINAL_AGG: Uuid = uuid!("00000000-0000-0000-0000-00000000000e");
const PHYS_LIMIT: Uuid = uuid!("00000000-0000-0000-0000-00000000000f");
const PHYS_OUTPUT: Uuid = uuid!("00000000-0000-0000-0000-000000000010");

const PORT_LOG_SCAN_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000011");
const PORT_LOG_FILTER_IN: Uuid = uuid!("00000000-0000-0000-0000-000000000012");
const PORT_LOG_FILTER_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000013");
const PORT_LOG_AGGREGATE_IN: Uuid = uuid!("00000000-0000-0000-0000-000000000014");
const PORT_LOG_AGGREGATE_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000015");
const PORT_LOG_LIMIT_IN: Uuid = uuid!("00000000-0000-0000-0000-000000000016");
const PORT_LOG_LIMIT_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000017");
const PORT_LOG_OUTPUT_IN: Uuid = uuid!("00000000-0000-0000-0000-000000000018");

const PORT_PHYS_SCAN_FILTER_OUT: Uuid = uuid!("00000000-0000-0000-0000-000000000019");
const PORT_PHYS_PARTIAL_AGG_IN: Uuid = uuid!("00000000-0000-0000-0000-00000000001a");
const PORT_PHYS_PARTIAL_AGG_OUT: Uuid = uuid!("00000000-0000-0000-0000-00000000001b");
const PORT_PHYS_FINAL_AGG_IN: Uuid = uuid!("00000000-0000-0000-0000-00000000001c");
const PORT_PHYS_FINAL_AGG_OUT: Uuid = uuid!("00000000-0000-0000-0000-00000000001d");
const PORT_PHYS_LIMIT_IN: Uuid = uuid!("00000000-0000-0000-0000-00000000001e");
const PORT_PHYS_LIMIT_OUT: Uuid = uuid!("00000000-0000-0000-0000-00000000001f");
const PORT_PHYS_OUTPUT_IN: Uuid = uuid!("00000000-0000-0000-0000-000000000020");

macro_rules! ts {
    ($ts:expr, $($body:tt)+) => {{
        ::quent_time::set_timestamp($ts);
        { $($body)+ }
    }};
    ($ts:expr) => { ::quent_time::set_timestamp($ts) };
}

#[derive(Parser, Debug)]
#[command(name = "quent-query-engine-fixed")]
#[command(about = "Emits a deterministic query-engine telemetry stream", long_about = None)]
struct Args {
    #[arg(long, default_value = "collector")]
    exporter: String,

    #[arg(
        long,
        default_value = "http://localhost:7836",
        env = "QUENT_COLLECTOR_ADDRESS"
    )]
    collector_address: String,

    #[arg(long, default_value = "data")]
    output_dir: String,
}

fn emit(ctx: &FixedContext) {
    let engine_obs = ctx.engine_observer();
    let worker_obs = ctx.worker_observer();
    let group_obs = ctx.query_group_observer();
    let query_obs = ctx.query_observer();
    let plan_obs = ctx.plan_observer();
    let op_obs = ctx.operator_observer();
    let port_obs = ctx.port_observer();

    // Setup
    ts!(
        0,
        engine_obs.create(ENGINE).init(engine::Init {
            instance_name: Some("demo-engine".into()),
            implementation: EngineImplementationAttributes {
                name: Some("Fixed".into()),
                version: Some("0.0.0".into()),
                custom_attributes: Default::default(),
            },
        })
    );
    ts!(
        1_000_000,
        worker_obs.create(WORKER).init(worker::Init {
            parent_engine_id: Ref::new(ENGINE),
            instance_name: "worker-0".into(),
        })
    );
    ts!(
        10_000_000,
        group_obs.declaration(
            QUERY_GROUP,
            query_group::Declaration {
                engine_id: ENGINE,
                instance_name: "TPC-H demo".into(),
            },
        )
    );

    // Query: init → planning → executing → exit
    let mut query = ts!(
        20_000_000,
        query_obs.init(QUERY, "Q0", Ref::new(QUERY_GROUP))
    );
    ts!(30_000_000, query.planning());

    // Logical plan: Scan → Filter → Aggregate → Limit → Output
    let logical_edges = vec![
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
        40_000_000,
        plan_obs.declaration(
            LOGICAL_PLAN,
            plan::Declaration {
                instance_name: "logical".into(),
                parent: plan::PlanParent {
                    query_id: Some(Ref::new(QUERY)),
                    plan_id: None,
                },
                worker_id: None,
                edges: logical_edges,
            },
        )
    );

    // Logical operator declarations
    let logical_ops: [(u64, Uuid, &str); 5] = [
        (40_001_000, LOG_SCAN, "Scan"),
        (40_002_000, LOG_FILTER, "Filter"),
        (40_003_000, LOG_AGGREGATE, "Aggregate"),
        (40_004_000, LOG_LIMIT, "Limit"),
        (40_005_000, LOG_OUTPUT, "Output"),
    ];
    for (t, id, name) in logical_ops {
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
    // Logical port declarations
    let logical_ports: [(u64, Uuid, Uuid, &str); 8] = [
        (40_006_000, PORT_LOG_SCAN_OUT, LOG_SCAN, "out"),
        (40_007_000, PORT_LOG_FILTER_IN, LOG_FILTER, "in"),
        (40_008_000, PORT_LOG_FILTER_OUT, LOG_FILTER, "out"),
        (40_009_000, PORT_LOG_AGGREGATE_IN, LOG_AGGREGATE, "in"),
        (40_010_000, PORT_LOG_AGGREGATE_OUT, LOG_AGGREGATE, "out"),
        (40_011_000, PORT_LOG_LIMIT_IN, LOG_LIMIT, "in"),
        (40_012_000, PORT_LOG_LIMIT_OUT, LOG_LIMIT, "out"),
        (40_013_000, PORT_LOG_OUTPUT_IN, LOG_OUTPUT, "in"),
    ];
    for (t, id, op, name) in logical_ports {
        ts!(
            t,
            port_obs.create(id).declaration(port::Declaration {
                operator_id: Ref::new(op),
                instance_name: name.into(),
            })
        );
    }

    // Physical plan: ScanFilter → PartialAggregate → FinalAggregate → Limit → Output
    let physical_edges = vec![
        plan::Edge {
            source: Ref::new(PORT_PHYS_SCAN_FILTER_OUT),
            target: Ref::new(PORT_PHYS_PARTIAL_AGG_IN),
        },
        plan::Edge {
            source: Ref::new(PORT_PHYS_PARTIAL_AGG_OUT),
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
        60_000_000,
        plan_obs.declaration(
            PHYSICAL_PLAN,
            plan::Declaration {
                instance_name: "physical".into(),
                parent: plan::PlanParent {
                    query_id: None,
                    plan_id: Some(Ref::new(LOGICAL_PLAN)),
                },
                worker_id: Some(Ref::new(WORKER)),
                edges: physical_edges,
            },
        )
    );

    let physical_ops: [(u64, Uuid, &str, &[Uuid]); 5] = [
        (
            60_001_000,
            PHYS_SCAN_FILTER,
            "ScanFilter",
            &[LOG_SCAN, LOG_FILTER],
        ),
        (
            60_002_000,
            PHYS_PARTIAL_AGG,
            "PartialAggregate",
            &[LOG_AGGREGATE],
        ),
        (
            60_003_000,
            PHYS_FINAL_AGG,
            "FinalAggregate",
            &[LOG_AGGREGATE],
        ),
        (60_004_000, PHYS_LIMIT, "Limit", &[LOG_LIMIT]),
        (60_005_000, PHYS_OUTPUT, "Output", &[LOG_OUTPUT]),
    ];
    for (t, id, name, parents) in physical_ops {
        ts!(
            t,
            op_obs.create(id).declaration(operator::Declaration {
                plan_id: Ref::new(PHYSICAL_PLAN),
                parent_operator_ids: parents.iter().map(|p| Ref::new(*p)).collect(),
                instance_name: name.into(),
                type_name: name.into(),
                custom_attributes: Default::default(),
            })
        );
    }
    let physical_ports: [(u64, Uuid, Uuid, &str); 8] = [
        (
            60_006_000,
            PORT_PHYS_SCAN_FILTER_OUT,
            PHYS_SCAN_FILTER,
            "out",
        ),
        (60_007_000, PORT_PHYS_PARTIAL_AGG_IN, PHYS_PARTIAL_AGG, "in"),
        (
            60_008_000,
            PORT_PHYS_PARTIAL_AGG_OUT,
            PHYS_PARTIAL_AGG,
            "out",
        ),
        (60_009_000, PORT_PHYS_FINAL_AGG_IN, PHYS_FINAL_AGG, "in"),
        (60_010_000, PORT_PHYS_FINAL_AGG_OUT, PHYS_FINAL_AGG, "out"),
        (60_011_000, PORT_PHYS_LIMIT_IN, PHYS_LIMIT, "in"),
        (60_012_000, PORT_PHYS_LIMIT_OUT, PHYS_LIMIT, "out"),
        (60_013_000, PORT_PHYS_OUTPUT_IN, PHYS_OUTPUT, "in"),
    ];
    for (t, id, op, name) in physical_ports {
        ts!(
            t,
            port_obs.create(id).declaration(port::Declaration {
                operator_id: Ref::new(op),
                instance_name: name.into(),
            })
        );
    }

    ts!(80_000_000, query.executing());

    // Statistics — empty custom_attributes; mnemonic decoder is just UUIDs + timestamps.
    let op_stats: [(u64, Uuid); 10] = [
        (100_000_000, LOG_SCAN),
        (101_000_000, LOG_FILTER),
        (102_000_000, LOG_AGGREGATE),
        (103_000_000, LOG_LIMIT),
        (104_000_000, LOG_OUTPUT),
        (105_000_000, PHYS_SCAN_FILTER),
        (106_000_000, PHYS_PARTIAL_AGG),
        (107_000_000, PHYS_FINAL_AGG),
        (108_000_000, PHYS_LIMIT),
        (109_000_000, PHYS_OUTPUT),
    ];
    for (t, op_id) in op_stats {
        ts!(
            t,
            op_obs.create(op_id).statistics(operator::Statistics {
                custom_attributes: Default::default(),
            })
        );
    }
    let port_stats: [(u64, Uuid); 16] = [
        (110_000_000, PORT_LOG_SCAN_OUT),
        (111_000_000, PORT_LOG_FILTER_IN),
        (112_000_000, PORT_LOG_FILTER_OUT),
        (113_000_000, PORT_LOG_AGGREGATE_IN),
        (114_000_000, PORT_LOG_AGGREGATE_OUT),
        (115_000_000, PORT_LOG_LIMIT_IN),
        (116_000_000, PORT_LOG_LIMIT_OUT),
        (117_000_000, PORT_LOG_OUTPUT_IN),
        (118_000_000, PORT_PHYS_SCAN_FILTER_OUT),
        (119_000_000, PORT_PHYS_PARTIAL_AGG_IN),
        (120_000_000, PORT_PHYS_PARTIAL_AGG_OUT),
        (121_000_000, PORT_PHYS_FINAL_AGG_IN),
        (122_000_000, PORT_PHYS_FINAL_AGG_OUT),
        (123_000_000, PORT_PHYS_LIMIT_IN),
        (124_000_000, PORT_PHYS_LIMIT_OUT),
        (125_000_000, PORT_PHYS_OUTPUT_IN),
    ];
    for (t, port_id) in port_stats {
        ts!(
            t,
            port_obs.create(port_id).statistics(port::Statistics {
                custom_attributes: Default::default(),
            })
        );
    }

    // Teardown: query → worker → engine
    ts!(200_000_000, query.exit());
    ts!(300_000_000, worker_obs.create(WORKER).exit(worker::Exit));
    ts!(400_000_000, engine_obs.create(ENGINE).exit(engine::Exit));
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

    let ctx = FixedContext::try_new(ENGINE, exporter)?;
    emit(&ctx);
    drop(ctx);
    Ok(())
}
