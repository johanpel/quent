// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use quent_attributes::Attribute;
use quent_v2_model::{
    exporter::{ExporterOptions, NdjsonExporterOptions},
    CapacityValue, EntityHandle, OccupancyBound, Usage,
};
use quent_v2_readme_example::*;
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = std::path::PathBuf::from("./events");
    let exporter = ExporterOptions::Ndjson(NdjsonExporterOptions {
        output_dir: output_dir.clone(),
    });
    let id = Uuid::now_v7();
    let context = AppContext::try_new(id, Some(exporter))?;

    // Create all observers up front.
    let cluster_obs = context.cluster_observer();
    let worker_obs = context.worker_observer();
    let queue_obs = context.queue_observer();
    let memory_pool_obs = context.memory_pool_observer();
    let thread_obs = context.thread_observer();
    let info_obs = context.info_observer();
    let file_stats_obs = context.file_stats_observer();
    let task_obs = context.task_observer();

    // Create the root resource group.
    let cluster = cluster_obs.cluster(Cluster {
        instance_name: "example_cluster".to_string(),
    })?;

    // Spawn a worker.
    let worker = worker_obs.worker(Worker {
        instance_name: "worker_0".to_string(),
        details: Details {
            version: "42.1.2".to_string(),
            custom: vec![Attribute::u64("threads", 256)].into(),
        },
        parent: cluster.into(),
    })?;

    // Construct a queue.
    let queue = queue_obs
        .init(QueueInit {
            parent_group_id: worker.id,
        })?
        .operating()?;

    // Construct a memory pool.
    let mut mem_pool = memory_pool_obs
        .init(MemoryPoolInit {
            parent_group_id: worker.id,
        })?
        .operating(MemoryPoolOperating {
            bytes: OccupancyBound { value: 1337 },
        })?;
    mem_pool = mem_pool.resizing()?.operating(MemoryPoolOperating {
        bytes: OccupancyBound { value: 2048 },
    })?;

    // Spawn a thread.
    let thread = thread_obs
        .init(ThreadInit {
            parent_group_id: worker.id,
        })?
        .operating()?;

    // Single event entity
    info_obs.info(Info {
        message: "ready to operate".to_string(),
        source: Some(file!().to_string()),
    })?;

    // Multi-event entities can emit in any order from an entity handle.
    let file_stats = file_stats_obs.handle()?;
    file_stats.checksum(Checksum {
        algorithm: "sha256".to_string(),
        value: "abc123def456".to_string(),
    })?;
    file_stats.decompressed(Decompressed {
        algorithm: "snappy".to_string(),
        ratio: 0.4,
    })?;

    // Queue a task. The entry transition returns an FSM handle.
    let task = task_obs.queued(Queued {
        instance_name: "my_task_31415".to_string(),
        index: 1,
        worker,
        queue: Some(Usage {
            instance: queue.id(),
            amounts: QueueUsage {
                entries: CapacityValue { value: 1 },
            },
        }),
    })?;

    let task = task.computing(Computing {
        thread: Some(Usage {
            instance: thread.id(),
            amounts: ThreadUsage,
        }),
        memory: None,
    })?;

    let task = task.computing(Computing {
        thread: Some(Usage {
            instance: thread.id(),
            amounts: ThreadUsage,
        }),
        memory: Some(Usage {
            instance: mem_pool.id(),
            amounts: MemoryPoolUsage {
                bytes: CapacityValue { value: 1024 },
            },
        }),
    })?;

    task.exit()?;

    // Finalize and exit the resources, in reverse construction order.
    thread.finalizing()?.exit()?;
    mem_pool.finalizing()?.exit()?;
    queue.finalizing()?.exit()?;

    // Drop context to flush all pending events.
    drop(context);

    let output_path = output_dir.join(format!("{id}.ndjson"));
    println!(
        "Events written to: {}",
        output_path.canonicalize()?.display()
    );

    Ok(())
}
