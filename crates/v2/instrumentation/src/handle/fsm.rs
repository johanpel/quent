use uuid::Uuid;

use crate::{EventSender, ObserverError, handle::Handle};

pub type SequenceNumber = u16;

#[derive(Debug)]
pub enum Transition<T> {
    Normal {
        sequence_number: SequenceNumber,
        payload: T,
    },
    Exit {
        sequence_number: SequenceNumber,
    },
}

pub struct FsmHandle<T> {
    inner: Handle<Transition<T>>,
    sequence: std::sync::atomic::AtomicU16,
}

impl<T> FsmHandle<T> {
    pub fn new(tx: EventSender<Transition<T>>, id: Uuid) -> Self {
        Self {
            inner: Handle::new(tx, id),
            sequence: std::sync::atomic::AtomicU16::new(0),
        }
    }

    pub fn id(&self) -> Uuid {
        self.inner.id()
    }

    pub fn emit_normal(&self, payload: T) -> Result<(), ObserverError> {
        let seq = self.next_sequence_number();
        self.inner.emit(Transition::Normal {
            sequence_number: seq,
            payload,
        })
    }

    pub fn emit_exit(&self) -> Result<(), ObserverError> {
        let seq = self.next_sequence_number();
        self.inner.emit(Transition::Exit {
            sequence_number: seq,
        })
    }

    fn next_sequence_number(&self) -> u16 {
        self.sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        // TODO(johanpel): consider error on wrap
    }
}
