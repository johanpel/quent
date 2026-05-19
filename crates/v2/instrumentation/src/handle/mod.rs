use uuid::Uuid;

use crate::{EventSender, ObserverError};

pub mod fsm;

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
