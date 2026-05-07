use std::marker::PhantomData;

use crate::v2::entity::{EntityDeclaration, EntityHandle, ObserverError};
use crate::v2::resource::{CapacityValue, OccupancyBound, Usage, memory, thread};
use quent_model_macros::Fsm;

use uuid::Uuid;

mod task {
    use super::*;

    mod model {
        use super::*;

        // The above needs to generate a type that can convey a run-time value of the amount of usage.
        // In this case, it is just the unit type since this is a unit resource.

        // Attribute set for a state with a resource usage.
        pub struct Computing {
            pub thread: Usage<thread::model::Thread>,
            pub global_memory: Usage<memory::model::Memory>,
            pub pool_memory: Option<Usage<memory::model::Memory>>, // optional usage is allowed
        }

        #[derive(Fsm)]
        #[quent(transitions = {
            entry -> Queueing,
            Queueing -> Computing,
            Computing -> exit
        })]
        pub enum Task {
            Queueing,
            Computing(Computing),
        }
    }

    mod desugared {}

    mod instrumentation {
        use super::*;

        // Tag type generated to support the type-state pattern below
        pub struct Queueing;
        pub struct Computing;

        pub struct TaskObserver {
            // holds same stuff as in entity examples
        }
        impl TaskObserver {
            pub fn queueing(&self) -> Result<TaskHandle<Queueing>, ObserverError> {
                todo!()
            }
        }

        pub struct TaskHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
        }
        impl EntityDeclaration for model::Task {}
        impl<T> EntityHandle for TaskHandle<T> {
            type DeclarationType = model::Task;
            fn id(&self) -> Uuid {
                self.id
            }
        }
        impl TaskHandle<Queueing> {
            pub fn computing(
                self,
                _attributes: model::Computing,
            ) -> Result<TaskHandle<Computing>, ObserverError> {
                todo!()
            }
        }
        impl TaskHandle<Computing> {
            pub fn exit(self) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }

    mod usage {
        use super::*;

        fn example() -> std::result::Result<(), Box<dyn std::error::Error>> {
            let thread_obs = thread::instrumentation::ThreadObserver {};
            let memory_obs = memory::instrumentation::MemoryObserver {};

            let task_obs = instrumentation::TaskObserver {};

            let mem_global_inst = memory_obs
                .init(memory::desugared::MemoryInit {
                    parent_group_id: Uuid::nil(),
                })?
                .operating(memory::desugared::MemoryOperating {
                    bytes: OccupancyBound { value: 1337 },
                })?;
            let thread_inst = thread_obs
                .init(thread::desugared::ThreadInit {
                    parent_group_id: Uuid::nil(),
                })?
                .operating()?;

            task_obs
                .queueing()?
                .computing(model::Computing {
                    thread: Usage {
                        instance: thread_inst.id(),
                        amounts: thread::instrumentation::ThreadUsage {},
                    },
                    global_memory: Usage {
                        instance: mem_global_inst.id(),
                        amounts: memory::instrumentation::MemoryUsage {
                            bytes: CapacityValue { value: 100u64 },
                        },
                    },
                    pool_memory: None,
                })?
                .exit()?;

            Ok(())
        }
    }
}
