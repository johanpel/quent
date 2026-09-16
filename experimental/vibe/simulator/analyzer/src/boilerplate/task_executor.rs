// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[derive(Default)]
pub(crate) struct TaskExecutorAccumulator {
    instance_name: Option<String>,
    worker_id: Option<Uuid>,
}

impl quent_events::Entity for TaskExecutorAccumulator {
    type Event = schema::TaskExecutorEvent;
}

impl EntityEventAccumulator for TaskExecutorAccumulator {
    fn push(&mut self, event: Self::Event) {
        let schema::TaskExecutorEvent::Declaration {
            instance_name,
            worker_id,
        } = event;
        self.instance_name = Some(instance_name);
        self.worker_id = Some(worker_id.target);
    }
}

pub(crate) struct TaskExecutor(AnalyzedEntity<TaskExecutorAccumulator>);

impl TaskExecutor {
    pub(crate) fn try_from_event(event: Event<schema::TaskExecutorEvent>) -> AnalyzerResult<Self> {
        Ok(Self(AnalyzedEntity::try_from_first_event(event)?))
    }

    pub(crate) fn push(&mut self, event: Event<schema::TaskExecutorEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }
}

impl Entity for TaskExecutor {
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
}

impl RefTreeEntity for TaskExecutor {
    fn parent_id(&self) -> Option<Uuid> {
        self.0.accumulator().worker_id
    }
}

impl ResourceGroup for TaskExecutor {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_id()
    }
}
