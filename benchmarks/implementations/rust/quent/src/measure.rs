// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use quent_bench_rust_common::{WorkloadArgs, make_case_result, measure_threads};
use quent_bench_types::{EventShape, Implementation};
use quent_instrumentation::{
    Context, EventModel, ExporterOptions, FileSystemExporterOptions, HandleError,
    InstrumentedEntity, InstrumentedModel, Noop, ObserverBuilder, ObserverProvider,
    build_info::ModelSource,
};

use crate::models;
use crate::verify;
use crate::{BenchResult, CaseResult, Exporter};

pub fn run_case(
    exporter: Exporter,
    shape: EventShape,
    workload: WorkloadArgs,
) -> BenchResult<CaseResult> {
    let directory = exporter
        .file_format()
        .map(|_| tempfile::tempdir())
        .transpose()?;
    let export_root = directory.as_ref().map(|directory| directory.path());
    let durations = match shape {
        EventShape::Empty => {
            run_typed::<models::empty::LatencyEmpty, models::empty::Entity, _, _, _>(
                exporter,
                export_root,
                workload,
                |_| (),
                |handle, ()| handle.instr_call(),
            )?
        }
        EventShape::U8 => run_typed::<models::u8::LatencyU8, models::u8::Entity, _, _, _>(
            exporter,
            export_root,
            workload,
            |index| index as u8,
            |handle, value| handle.instr_call(value),
        )?,
        EventShape::U64 => run_typed::<models::u64::LatencyU64, models::u64::Entity, _, _, _>(
            exporter,
            export_root,
            workload,
            |index| index,
            |handle, value| handle.instr_call(value),
        )?,
        EventShape::ShortString => run_typed::<
            models::short_string::LatencyShortString,
            models::short_string::Entity,
            _,
            _,
            _,
        >(
            exporter,
            export_root,
            workload,
            short_string,
            |handle, value| handle.instr_call(value),
        )?,
        EventShape::LongString => {
            run_typed::<models::long_string::LatencyLongString, models::long_string::Entity, _, _, _>(
                exporter,
                export_root,
                workload,
                long_string,
                |handle, value| handle.instr_call(value),
            )?
        }
        EventShape::All => run_typed::<models::all::LatencyAll, models::all::Entity, _, _, _>(
            exporter,
            export_root,
            workload,
            |index| (index as u8, index, short_string(index), long_string(index)),
            |handle, (small, large, short, long)| handle.instr_call(small, large, short, long),
        )?,
    };
    Ok(make_case_result(
        Implementation::Quent,
        Some(exporter),
        Some(shape),
        workload,
        durations,
    ))
}

fn short_string(index: u64) -> String {
    "s".repeat(8 + (index % 9) as usize)
}

fn long_string(index: u64) -> String {
    "l".repeat(128 + (index % 129) as usize)
}

fn run_typed<M, E, P, PrepareFn, EmitFn>(
    exporter: Exporter,
    export_root: Option<&Path>,
    workload: WorkloadArgs,
    prepare: PrepareFn,
    emit: EmitFn,
) -> BenchResult<Vec<Vec<u64>>>
where
    M: EventModel
        + InstrumentedModel
        + ModelSource
        + ObserverBuilder<Noop>
        + ObserverBuilder<ExporterOptions>,
    M::Observers: ObserverProvider<E>,
    E: InstrumentedEntity<Context = Context<M>>,
    E::Handle: Send + 'static,
    P: Send + 'static,
    PrepareFn: Fn(u64) -> P + Copy + Send + 'static,
    EmitFn: Fn(&E::Handle, P) -> Result<(), HandleError> + Copy + Send + 'static,
{
    let context = match exporter.file_format() {
        None => Context::<M>::try_new(Noop)?,
        Some(format) => {
            Context::<M>::try_new(ExporterOptions::FileSystem(FileSystemExporterOptions::new(
                format,
                export_root
                    .ok_or("filesystem export root is missing")?
                    .to_path_buf(),
            )))?
        }
    };
    let context_id = context.id();
    let observer = context.observer::<E>();
    let durations = measure_threads(workload, || observer.handle(), prepare, emit)?;
    drop(observer);
    drop(context);

    verify::exported_events(
        exporter,
        export_root,
        context_id,
        workload.threads.get(),
        workload.batch_config(),
        workload.preflight_call(),
    )?;
    Ok(durations)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::num::{NonZeroU64, NonZeroUsize};

    use super::*;

    #[test]
    fn combined_event_exports_every_payload_length() {
        let directory = tempfile::tempdir().unwrap();
        let durations = run_typed::<models::all::LatencyAll, models::all::Entity, _, _, _>(
            Exporter::Ndjson,
            Some(directory.path()),
            WorkloadArgs {
                threads: NonZeroUsize::MIN,
                num_batches: NonZeroUsize::new(2).unwrap(),
                batch_size: NonZeroU64::new(129).unwrap(),
                num_warmup_batches: 1,
                batch_pause_interval_us: 0,
                no_preflight_call: true,
            },
            |index| (index as u8, index, short_string(index), long_string(index)),
            |handle, (small, large, short, long)| handle.instr_call(small, large, short, long),
        )
        .unwrap();
        assert_eq!(durations.len(), 1);
        assert_eq!(durations[0].len(), 2);

        let context = std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let entity = context.join("Entity");
        let file = std::fs::read_dir(entity)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let records: Vec<serde_json::Value> = BufReader::new(std::fs::File::open(file).unwrap())
            .lines()
            .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
            .collect();
        assert_eq!(records.len(), 3 * 129);
        for (index, record) in records.iter().enumerate() {
            let payload = &record["data"]["InstrCall"];
            assert_eq!(payload["small"], index as u8);
            assert_eq!(payload["large"], index as u64);
            assert_eq!(payload["short"].as_str().unwrap().len(), 8 + index % 9);
            assert_eq!(payload["long"].as_str().unwrap().len(), 128 + index % 129);
            assert!(record["id"].is_string());
            assert!(record["timestamp"].is_number());
        }
    }
}
