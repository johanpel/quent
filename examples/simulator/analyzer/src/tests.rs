// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Mutex};

use quent_analyzer::Entity;
use quent_instrumentation::EventCallback;
use quent_query_engine_analyzer::ui::UiAnalyzer;
use quent_simulator_instrumentation as instr;
use uuid::Uuid;

use crate::{QueryEngineModel, SimulatorUiAnalyzer};

#[test]
fn analyzes_events_emitted_by_schema_context() {
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let callback = {
        let recorded = Arc::clone(&recorded);
        EventCallback::new(move |event| recorded.lock().unwrap().push(event))
    };

    let engine_id = Uuid::now_v7();
    let query_id = Uuid::now_v7();
    {
        let context = instr::Context::<instr::Simulator>::try_new(callback).unwrap();
        let mut engine = context
            .observer::<instr::Engine>()
            .handle_with_id(engine_id);
        engine
            .init(
                instr::EngineImplementationAttributes {
                    name: Some("test".to_owned()),
                    version: Some("1".to_owned()),
                    custom_attributes: Default::default(),
                },
                Some("engine".to_owned()),
            )
            .unwrap();

        let mut worker = context.observer::<instr::Worker>().handle();
        worker
            .init(engine.as_entity_ref(), "worker".to_owned())
            .unwrap();

        let mut query_group = context.observer::<instr::QueryGroup>().handle();
        query_group
            .declaration("group".to_owned(), engine.as_entity_ref())
            .unwrap();

        let mut query = context.observer::<instr::Query>().handle_with_id(query_id);
        query
            .init("query".to_owned(), query_group.as_entity_ref())
            .unwrap();
        query.planning().unwrap();

        let mut plan = context.observer::<instr::Plan>().handle();
        plan.declaration(
            instr::PlanParent {
                query_id: query.as_entity_ref(),
                plan_id: None,
            },
            "plan".to_owned(),
            vec![],
            Some(worker.as_entity_ref()),
        )
        .unwrap();
        let mut operator = context.observer::<instr::Operator>().handle();
        operator
            .declaration(
                plan.as_entity_ref(),
                vec![],
                "operator".to_owned(),
                "scan".to_owned(),
                Default::default(),
            )
            .unwrap();

        let mut thread_pool = context.observer::<instr::ThreadPool>().handle();
        thread_pool
            .declaration("threads".to_owned(), worker.as_entity_ref())
            .unwrap();
        let mut processor = context.observer::<instr::Processor>().handle();
        processor
            .initializing("thread".to_owned(), thread_pool.as_entity_ref())
            .unwrap();
        processor.operating().unwrap();
        let mut memory = context.observer::<instr::Memory>().handle();
        memory
            .initializing("memory".to_owned(), worker.as_entity_ref())
            .unwrap();
        memory
            .operating(instr::MemoryBounds { bytes: 1024 })
            .unwrap();

        let mut task = context.observer::<instr::Task>().handle();
        task.queueing(operator.as_entity_ref()).unwrap();
        task.allocating(processor.as_entity_ref_with(instr::ProcessorUsage))
            .unwrap();
        task.computing(
            128,
            processor.as_entity_ref_with(instr::ProcessorUsage),
            memory.as_entity_ref_with(instr::MemoryUsage { bytes: 128 }),
        )
        .unwrap();
        task.exit().unwrap();

        query.executing().unwrap();
        query.exit().unwrap();
        memory.finalizing().unwrap();
        memory.exit().unwrap();
        processor.finalizing().unwrap();
        processor.exit().unwrap();
        worker.exit().unwrap();
        engine.exit().unwrap();
    }

    let events = std::mem::take(&mut *recorded.lock().unwrap());
    let analyzer = SimulatorUiAnalyzer::try_new(engine_id, events.into_iter()).unwrap();

    assert_eq!(analyzer.model.queries().count(), 1);
    assert_eq!(
        analyzer.model.query(query_id).unwrap().instance_name(),
        "query"
    );
    assert_eq!(analyzer.model.tasks.len(), 1);
    assert_eq!(analyzer.model.arbitrary_resources.resources.len(), 2);
}
