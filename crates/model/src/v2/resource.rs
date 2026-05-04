use std::marker::PhantomData;

use quent_model_macros::Fsm;
use quent_model_macros::Resource;
use uuid::Uuid;

use crate::v2::entity::ObserverError;

// Notes:
//
// Resources should roughly be considered an attribute "convention" on top of
// the entity and FSM semantics. As such it should be possible to provide a
// sugaring syntax over those concepts, without requiring additional core things
// from the entity and FSM derives.

// User-facing types used during modeling
struct Capacity<T> {
    _phantom: PhantomData<T>,
}

// a unit resource
mod thread {
    use super::*;

    mod model {
        use super::*;

        // Unit resource
        #[derive(Resource)] // Derive macro to be implemented
        struct Thread;
    }

    mod desugared {
        use super::*;

        pub struct ThreadInit {
            parent_group_id: Uuid,
        }

        #[derive(Fsm)]
        pub enum ThreadFsm {
            Init(ThreadInit),
            Operating,
            Finalizing,
            Exit,
        }
    }

    // Since a resource is an FSM with predefined transition, no additional types need to be generated besides pub enum ThreadFsm.
    mod events {}

    mod instrumentation {
        use super::*;

        struct Init;
        struct Operating;
        struct Finalizing;

        struct ThreadObserver {}
        impl ThreadObserver {
            fn init(
                &self,
                _attributes: desugared::ThreadInit,
            ) -> Result<ThreadHandle<Init>, ObserverError> {
                // clones sender into handle
                // emits state transition event
                todo!()
            }
        }

        struct ThreadHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            // + sender to event channel of the observer
        }

        impl ThreadHandle<Init> {
            fn operating(self) -> Result<ThreadHandle<Operating>, ObserverError> {
                // emits event
                todo!()
            }
        }

        impl ThreadHandle<Operating> {
            fn finalizing(self) -> Result<ThreadHandle<Finalizing>, ObserverError> {
                // emits event
                todo!()
            }
        }

        impl ThreadHandle<Finalizing> {
            fn exit(self) -> Result<(), ObserverError> {
                // emits event
                todo!()
            }
        }
    }

    mod analysis {}
}

// a resource with one occupancy capacity
mod memory {
    use super::*;

    mod model {
        use super::*;

        // While a resource is an FSM, it has a very constrained declaration
        // space compared to arbitrary FSMs. The only thing we need to declare
        // are its capacities.
        //
        // Since at some point in time, all available capacities of a resource
        // are simultaneously used by things to a certain amount, so using a
        // product type to declare it here a makes sense.
        #[derive(Resource)]
        pub struct Memory {
            pub bytes: Capacity<u64>,
        }
    }

    mod desugared {
        use super::*;

        #[derive(Fsm)]
        pub enum MemoryFsm {}
    }

    mod events {}

    mod instrumentation {
        use super::*;

        struct MemoryObserver {
            // holds sender to mpsc event channel
        }

        impl MemoryObserver {
            fn init(&self) -> MemoryHandle {
                // clones sender into handle
                // emits state transition event
                todo!()
            }
        }

        struct MemoryHandle {
            id: Uuid,
            // + sender to event channel of the observer
        }

        impl MemoryHandle {
            fn operating(self, capacity_bytes: u64) -> Self {
                // emits event
                todo!()
            }
            fn finalizing(self) -> Self {
                // emits event
                todo!()
            }
            fn exit(self) {
                // emits event
                todo!()
            }
        }
    }

    mod analysis {
        use super::*;
        // TODO
    }
}

mod memory_resizable {
    use super::*;

    mod model {
        use super::*;

        // Resource with resizable capacity
        #[derive(Resource)]
        struct MemoryResizable {
            #[quent(resizable)] // means it will have the operating <-> resizing transitions
            bytes: Capacity<u64>,
        }
    }

    mod events {
        use super::*;

        struct MemoryResizableInit {
            parent_group_id: Uuid,
        }

        struct MemoryResizableOperating {
            capacity_bytes: u64,
        }

        enum MemoryResizableEvent {
            Init(MemoryResizableInit),
            Operating(MemoryResizableOperating),
            Resizing,
            Finalizing,
            Exit,
        }
    }

    mod instrumentation {
        use super::*;

        struct MemoryResizableObserver {
            // holds sender to mpsc event channel
        }

        impl MemoryResizableObserver {
            fn init(&self) -> MemoryResizableHandle {
                // clones sender into handle
                // emits state transition event
                todo!()
            }
        }

        struct MemoryResizableHandle {
            id: Uuid,
            // + sender to event channel of the observer
        }

        impl MemoryResizableHandle {
            fn operating(self, capacity_bytes: u64) -> Self {
                // emits event
                todo!()
            }
            fn resizing(self) -> Self {
                // emits event
                todo!()
            }
            fn finalizing(self) -> Self {
                // emits event
                todo!()
            }
            fn exit(self) {
                // emits event
                todo!()
            }
        }
    }

    mod analysis {
        use super::*;
        // TODO
    }
}

mod memory_unbounded {
    use super::*;

    mod model {
        use super::*;

        // Resource with potentially virtually unbounded capacity (i.e. we don't
        // know or don't care about out the maximum capacity at run time, just
        // want to track usage)
        //
        // If it is "unbounded", then the user is not required to set the
        // capacity bounds, but they can set the capacity bounds, if they know,
        // hence Option<u64> in the instrumentation API.
        #[derive(Resource)]
        struct MemoryUnbounded {
            #[quent(unbounded)]
            bytes: Capacity<u64>,
        }
    }

    mod events {
        use super::*;

        struct MemoryUnboundedInit {
            parent_group_id: Uuid,
        }

        struct MemoryUnboundedOperating {
            capacity_bytes: u64,
        }

        enum MemoryUnboundedEvent {
            Init(MemoryUnboundedInit),
            Operating(MemoryUnboundedOperating),
            Finalizing,
            Exit,
        }
    }

    mod instrumentation {
        use super::*;

        struct MemoryUnboundedObserver {
            // holds sender to mpsc event channel
        }

        impl MemoryUnboundedObserver {
            fn init(&self) -> MemoryUnboundedHandle {
                // clones sender into handle
                // emits state transition event
                todo!()
            }
        }

        struct MemoryUnboundedHandle {
            id: Uuid,
            // + sender to event channel of the observer
        }

        impl MemoryUnboundedHandle {
            fn operating(self, capacity_bytes: Option<u64>) -> Self {
                // emits event
                todo!()
            }
            fn finalizing(self) -> Self {
                // emits event
                todo!()
            }
            fn exit(self) {
                // emits event
                todo!()
            }
        }
    }

    mod analysis {
        use super::*;
        // TODO
    }
}

mod channel {
    use super::*;

    mod model {
        use super::*;

        // Resource with rate capacity
        //
        // For the resource operating state transition, the user is going to
        // convey the maximum number of items per unit of time that the resource
        // supports if the capacity is a "rate" kind of capacity. Thus we need
        // the transition to the resource operating state to make that clear by
        // its arguments, e.g. operating(bytes_per_second: f64).
        //
        // For an FSM entering a state in which this resource is used, the
        // amount of a "rate" kind capacity it uses is derived from the user
        // supplying the amount of items and the amount of time that amount of
        // items were used. But, the time dimension is captured implicitly by
        // the state transition events. So for an FSM transition with a usage of
        // this resource, say it is a network packet transferred over some
        // network channel, in the FSM transition event, they only have to
        // supply the size of the packet, rather than wait for the transition to
        // complete and calculate give the "bytes per second" number themselves
        // afterwards. Thus the fsm transition event API will simply be
        // something like transfer(bytes: u64).
        #[derive(Resource)]
        struct Channel {
            #[quent(rate, unbounded)]
            bytes: Capacity<u64>,
        }
    }

    mod events {
        use super::*;

        struct ChannelInit {
            parent_group_id: Uuid,
        }

        struct ChannelOperating {
            capacity_bytes_per_second: f64,
        }

        enum ChannelEvent {
            Init(ChannelInit),
            Operating(ChannelOperating),
            Finalizing,
            Exit,
        }
    }

    mod instrumentation {
        use super::*;

        struct ChannelObserver {
            // holds sender to mpsc event channel
        }

        impl ChannelObserver {
            fn init(&self) -> ChannelHandle {
                // clones sender into handle
                // emits state transition event
                todo!()
            }
        }

        struct ChannelHandle {
            id: Uuid,
            // + sender to event channel of the observer
        }

        impl ChannelHandle {
            fn operating(self, capacity_bytes_per_second: Option<f64>) -> Self {
                // emits event
                todo!()
            }
            fn finalizing(self) -> Self {
                // emits event
                todo!()
            }
            fn exit(self) {
                // emits event
                todo!()
            }
        }
    }

    mod analysis {
        use super::*;
        // TODO
    }
}

// in macros/codegen, not required, just a brainstorm, may need to look completely different:
// enum ValueType {
//     U64,
// }

// enum CapacityKind {
//     Occupancy,
//     Rate,
// }

// struct CapacityDecl {
//     name: String,
//     kind: CapacityKind,
//     typ: ValueType,
//     resizeable: bool,
//     unbounded: bool,
// }

// trait ResourceDecl {
//     fn capacities() -> impl Iterator<Item = CapacityDecl>;
// }

// impl ResourceDecl for Memory {
//     fn capacities() -> impl Iterator<Item = CapacityDecl> {
//         [CapacityDecl {
//             name: "bytes".to_owned(),      // from the field name
//             kind: CapacityKind::Occupancy, // implicit, occupancy unless there is #[quent(rate)]
//             typ: ValueType::U64,           // from the field type
//             resizeable: false,             // from #[quent(resizeable)]
//             unbounded: false,              // from field type (when using Option<T>)
//         }]
//         .into_iter()
//     }
// }

// impl ResourceDecl for MemoryResizable {
//     fn capacities() -> impl Iterator<Item = CapacityDecl> {
//         [CapacityDecl {
//             name: "bytes".to_owned(),
//             kind: CapacityKind::Occupancy,
//             typ: ValueType::U64,
//             resizeable: true,
//             unbounded: false,
//         }]
//         .into_iter()
//     }
// }

// impl ResourceDecl for MemoryUnbounded {
//     fn capacities() -> impl Iterator<Item = CapacityDecl> {
//         [CapacityDecl {
//             name: "bytes".to_owned(),
//             kind: CapacityKind::Occupancy,
//             typ: ValueType::U64,
//             resizeable: false,
//             unbounded: true,
//         }]
//         .into_iter()
//     }
// }
