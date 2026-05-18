use std::marker::PhantomData;

use uuid::Uuid;

pub use quent_exporter::ExporterOptions;

#[derive(Debug, thiserror::Error)]
pub enum ObserverError {
    #[error("todo")]
    Todo,
}

pub struct EventSender<T> {
    _t: PhantomData<T>,
}

impl<T> EventSender<T> {
    pub fn emit(&self, _id: Uuid, _payload: T) -> Result<(), ObserverError> {
        Ok(())
    }
}

impl<T> Clone for EventSender<T> {
    fn clone(&self) -> Self {
        Self { _t: PhantomData }
    }
}

pub struct Observer<T> {
    tx: EventSender<T>,
}

impl<T> Observer<T> {
    pub fn new(_root_id: Uuid, _opts: Option<ExporterOptions>) -> Result<Self, ObserverError> {
        Ok(Self {
            tx: EventSender { _t: PhantomData },
        })
    }

    pub fn sender(&self) -> EventSender<T> {
        self.tx.clone()
    }
}

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
}
