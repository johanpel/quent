// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use uuid::Uuid;

use crate::{EventSender, ObserverError};

pub mod fsm;

// TODO add trait bounds here
pub struct Handle<T> {
    tx: EventSender<T>,
    id: Uuid,
}

impl<T> Handle<T> {
    pub fn new(tx: EventSender<T>, id: Uuid) -> Self {
        Self { tx, id }
    }

    pub fn emit(&self, payload: T) -> Result<(), ObserverError> {
        self.tx.emit(self.id, payload)
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
}
