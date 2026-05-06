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

// TODO: seal traits below

// Trait + tag types for whether capacities are bounded or unbounded
trait Boundedness {}
/// The resource capacity is bounded.
pub struct Bounded;
impl Boundedness for Bounded {}
/// The resource capacity is unbounded.
///
/// It is physically always bounded, but the bounds may be unknown.
pub struct Unbounded;
impl Boundedness for Unbounded {}

// Trait + tag type for capacities that after resource init are either fixed or dynamically resizable.
trait Resizeability {}
/// The resource capacity is fixed after initialization.
pub struct Fixed;
impl Resizeability for Fixed {}
/// The resource capacity is resizable after initialization.
pub struct Resizable;
impl Resizeability for Resizable {}

// Trait + tag type for the kind of capacity.
trait CapacityKind {}
/// The resource capacity is fixed after initialization.
pub struct Occupancy;
impl CapacityKind for Occupancy {}
/// The resource capacity is resizable after initialization.
pub struct Rate;
impl CapacityKind for Rate {}

// User-facing types used during modeling While K, R, and B are two-valued
// properties, which would technically allow for the use of a const bool
// generic, it would make the declaration site less readable, hence we favor tag
// types.
//
// TODO: since not all combinations of R and B are allowed, consider making it a
// single three-valued generic.
//
// Would be nice if we could use plain enums as const generics, but we can't.
struct Capacity<T, K = Occupancy, R = Fixed, B = Bounded>
where
    K: CapacityKind,
    R: Resizeability,
    B: Boundedness,
{
    _value_type: PhantomData<T>,
    _kind: PhantomData<K>,
    _bounded: PhantomData<B>,
    _resizable: PhantomData<R>,
}

// User-facing types used in the instrumentation API:
/// To convey a new capacity bound.
struct CapacityBound<ValueType> {
    value: ValueType,
}

/// To convey a new capacity value.
struct CapacityValue<ValueType> {
    value: ValueType,
}

// A unit resource. Has one unnamed capacity with bound 1. Useful for threads,
// mutexes, or any other type of exclusive usage of a thing.
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
            Operating, // < nothing here since this is a unit resource. it's capacity is always exactly 1.
            Finalizing,
            Exit,
        }
    }

    // Since a resource is an FSM with predefined transition, no additional
    // types need to be generated besides pub enum ThreadFsm.
    mod events {}

    mod instrumentation {
        use super::*;

        pub struct Init;
        pub struct Operating;
        pub struct Finalizing;

        pub struct ThreadObserver {}

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

        pub struct ThreadHandle<T> {
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

// a resource with one bounded occupancy capacity
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

        pub struct MemoryInit {
            pub parent_group_id: Uuid,
        }

        pub struct MemoryOperating {
            pub bytes: CapacityBound<u64>,
        }

        #[derive(Fsm)]
        pub enum MemoryFsm {
            Init(MemoryInit),
            Operating(MemoryOperating),
            Finalizing,
        }
    }

    mod events {}

    mod instrumentation {
        use super::*;

        pub struct Init;
        pub struct Operating;
        pub struct Finalizing;

        pub struct MemoryObserver {}

        impl MemoryObserver {
            fn init(
                &self,
                _attributes: desugared::MemoryInit,
            ) -> Result<MemoryHandle<Init>, ObserverError> {
                // clones sender into handle
                // emits state transition event
                todo!()
            }
        }

        pub struct MemoryHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            // + sender to event channel of the observer
        }

        impl MemoryHandle<Init> {
            fn operating(
                self,
                _attributes: desugared::MemoryOperating,
            ) -> Result<MemoryHandle<Operating>, ObserverError> {
                // emits event
                todo!()
            }
        }

        impl MemoryHandle<Operating> {
            fn finalizing(self) -> Result<MemoryHandle<Finalizing>, ObserverError> {
                // emits event
                todo!()
            }
        }

        impl MemoryHandle<Finalizing> {
            fn exit(self) -> Result<(), ObserverError> {
                // emits event
                todo!()
            }
        }
    }

    mod analysis {
        // TODO
    }
}

// a resource with one unbounded occupancy capacity
mod memory_unbounded {
    use super::*;

    mod model {
        use super::*;

        // Resource with resizable capacity
        #[derive(Resource)]
        pub struct MemoryUnbounded {
            pub bytes: Capacity<u64, Occupancy, Fixed, Unbounded>,
        }
    }

    mod desugared {
        use super::*;

        pub struct MemoryUnboundedInit {
            pub parent_group_id: Uuid,
        }

        pub enum MemoryUnboundedFsm {
            Init(MemoryUnboundedInit),
            Operating, // nothing here, all capacities are unbounded
            Finalizing,
        }
    }

    mod events {}

    mod instrumentation {
        use super::*;

        pub struct Init;
        pub struct Operating;
        pub struct Resizing;
        pub struct Finalizing;

        pub struct MemoryUnboundedObserver {}

        impl MemoryUnboundedObserver {
            fn init(
                &self,
                _attributes: desugared::MemoryUnboundedInit,
            ) -> Result<MemoryUnboundedHandle<Init>, ObserverError> {
                // clones sender into handle
                // emits state transition event
                todo!()
            }
        }

        pub struct MemoryUnboundedHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            // + sender to event channel of the observer
        }

        impl MemoryUnboundedHandle<Init> {
            fn operating(self) -> Result<MemoryUnboundedHandle<Operating>, ObserverError> {
                // emits event
                todo!()
            }
        }
        impl MemoryUnboundedHandle<Operating> {
            fn finalizing(self) -> Result<MemoryUnboundedHandle<Finalizing>, ObserverError> {
                // emits event
                todo!()
            }
        }
        impl MemoryUnboundedHandle<Finalizing> {
            fn exit(self) -> Result<(), ObserverError> {
                // emits event
                todo!()
            }
        }
    }

    mod analysis {
        // TODO
    }
}

// a resource with one bounded occupancy capacity that is resizeable
mod memory_resizable {
    use super::*;

    mod model {
        use super::*;

        // Resource with resizable capacity
        #[derive(Resource)]
        pub struct MemoryResizable {
            pub bytes: Capacity<u64, Occupancy, Resizable>,
        }
    }

    mod desugared {
        use super::*;

        pub struct MemoryResizableInit {
            pub parent_group_id: Uuid,
        }

        pub struct MemoryResizableOperating {
            pub bytes: CapacityBound<u64>,
        }

        pub enum MemoryResizableFsm {
            Init(MemoryResizableInit),
            Operating(MemoryResizableOperating),
            Resizing, // additional state vs. resource without any resizable capacities.
            Finalizing,
        }
    }

    mod events {}

    mod instrumentation {
        use super::*;

        pub struct Init;
        pub struct Operating;
        pub struct Resizing;
        pub struct Finalizing;

        pub struct MemoryResizableObserver {}

        impl MemoryResizableObserver {
            fn init(
                &self,
                _attributes: desugared::MemoryResizableInit,
            ) -> Result<MemoryResizableHandle<Init>, ObserverError> {
                // clones sender into handle
                // emits state transition event
                todo!()
            }
        }

        pub struct MemoryResizableHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            // + sender to event channel of the observer
        }

        impl MemoryResizableHandle<Init> {
            fn operating(
                self,
                _attributes: desugared::MemoryResizableOperating,
            ) -> Result<MemoryResizableHandle<Operating>, ObserverError> {
                // emits event
                todo!()
            }
        }

        impl MemoryResizableHandle<Operating> {
            fn resizing(self) -> Result<MemoryResizableHandle<Operating>, ObserverError> {
                // emits event
                todo!()
            }
            fn finalizing(self) -> Result<MemoryResizableHandle<Finalizing>, ObserverError> {
                // emits event
                todo!()
            }
        }

        impl MemoryResizableHandle<Resizing> {
            fn operating(
                self,
                _attributes: desugared::MemoryResizableOperating,
            ) -> Result<MemoryResizableHandle<Operating>, ObserverError> {
                // emits event
                todo!()
            }
        }

        impl MemoryResizableHandle<Finalizing> {
            fn exit(self) -> Result<(), ObserverError> {
                // emits event
                todo!()
            }
        }
    }

    mod analysis {
        // TODO
    }
}

// a resource with one unbounded rate capacity
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
            bytes: Capacity<u64, Rate, Fixed, Unbounded>,
        }
    }

    mod desugared {
        use super::*;

        pub struct ChannelInit {
            pub parent_group_id: Uuid,
        }

        pub enum ChannelFsm {
            Init(ChannelInit),
            Operating,
            Resizing, // additional state vs. resource without any resizable capacities.
            Finalizing,
        }
    }

    mod events {}

    mod instrumentation {
        use super::*;

        pub struct Init;
        pub struct Operating;
        pub struct Resizing;
        pub struct Finalizing;

        pub struct ChannelObserver {}

        impl ChannelObserver {
            fn init(
                &self,
                _attributes: desugared::ChannelInit,
            ) -> Result<ChannelHandle<Init>, ObserverError> {
                // clones sender into handle
                // emits state transition event
                todo!()
            }
        }

        pub struct ChannelHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            // + sender to event channel of the observer
        }

        impl ChannelHandle<Init> {
            fn operating(self) -> Result<ChannelHandle<Operating>, ObserverError> {
                // emits event
                todo!()
            }
        }
        impl ChannelHandle<Operating> {
            fn finalizing(self) -> Result<ChannelHandle<Finalizing>, ObserverError> {
                // emits event
                todo!()
            }
        }
        impl ChannelHandle<Finalizing> {
            fn exit(self) -> Result<(), ObserverError> {
                // emits event
                todo!()
            }
        }
    }

    mod analysis {
        // TODO
    }
}

// things that should produce compilation errors
mod invalid {
    use super::*;

    mod model {
        use super::*;

        // A capacity can't be both resizable and unbounded. Resizing implies
        // the resource knows it's going over some limit and that a new limit
        // can be known.
        #[derive(Resource)]
        pub struct Invalid_0 {
            pub bytes: Capacity<u64, Occupancy, Resizable, Unbounded>,
        }
    }
}
