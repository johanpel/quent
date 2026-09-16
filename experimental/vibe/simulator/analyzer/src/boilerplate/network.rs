// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[derive(Default)]
pub(crate) struct NetworkAccumulator {
    instance_name: Option<String>,
    engine_id: Option<Uuid>,
}

impl quent_events::Entity for NetworkAccumulator {
    type Event = schema::NetworkEvent;
}

impl EntityEventAccumulator for NetworkAccumulator {
    fn push(&mut self, event: Self::Event) {
        let schema::NetworkEvent::Declaration {
            instance_name,
            engine_id,
        } = event;
        self.instance_name = Some(instance_name);
        self.engine_id = Some(engine_id.target);
    }
}

pub(crate) struct Network(AnalyzedEntity<NetworkAccumulator>);

impl Network {
    pub(crate) fn try_from_event(event: Event<schema::NetworkEvent>) -> AnalyzerResult<Self> {
        Ok(Self(AnalyzedEntity::try_from_first_event(event)?))
    }

    pub(crate) fn push(&mut self, event: Event<schema::NetworkEvent>) -> AnalyzerResult<()> {
        self.0.push(event)
    }
}

impl Entity for Network {
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

impl RefTreeEntity for Network {
    fn parent_id(&self) -> Option<Uuid> {
        self.0.accumulator().engine_id
    }
}

impl ResourceGroup for Network {
    fn parent_group_id(&self) -> Option<Uuid> {
        self.parent_id()
    }
}
