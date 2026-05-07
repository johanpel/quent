// An entity can be a resource group, which means that at least one of its
// events needs to carry resource group attributes.

use crate::v2::{
    entity::{EntityDeclaration, EntityHandle, Event, ObserverError},
    fsm::Transition,
};
use quent_model_macros::{Entity, Fsm, ResourceGroup, RootResourceGroup};
use quent_time::timestamp;

use std::{marker::PhantomData, sync::atomic::AtomicU16};

use uuid::Uuid;

pub struct ResourceGroupAttributes {
    pub parent_group_id: Uuid,
}

// Two types of entities as a non-root resource group.
mod entity_rg {
    use super::*;

    mod model {
        use super::*;

        // Single-event entity RG.
        //
        // The event payload will need the ResourceGroupAttributes in addition,
        // so we put them in a tuple.
        #[derive(Entity, ResourceGroup)]
        pub struct OneShot {
            pub value: u64,
        }

        pub struct X {
            pub foo: u64,
        }

        // Multi-event entity RG.
        //
        // The challenge here is to enforce the rule that one of the events must
        // carry the necessary attributes. We can only reason about that from
        // the tokens this derive macro receives.
        //
        // The most trivial way to solve this right now would seem to enforce
        // one of the enum variants to hold a tuple in which
        // ResourceGroupAttributes appears. This way we also know which event
        // carries it and how, for potential analysis types we generate in
        // future work.
        #[derive(Entity, ResourceGroup)]
        pub enum MultiOneShot {
            A(X, ResourceGroupAttributes),
            B(X),
        }
    }

    mod events {}

    mod instrumentation {
        use crate::v2::entity::EntityHandle;

        use super::*;

        pub struct OneShotObserver {}
        impl OneShotObserver {
            pub fn one_shot(
                &self,
                _attributes: (model::OneShot, ResourceGroupAttributes),
            ) -> Result<Uuid, ObserverError> {
                todo!()
            }
        }

        pub struct MultiOneShotObserver {}
        impl MultiOneShotObserver {
            pub fn handle(&self) -> Result<MultiOneShotHandle, ObserverError> {
                todo!()
            }
        }
        pub struct MultiOneShotHandle {
            id: Uuid,
        }
        impl EntityDeclaration for model::MultiOneShot {}
        impl EntityHandle for MultiOneShotHandle {
            type DeclarationType = model::MultiOneShot;
            fn id(&self) -> Uuid {
                self.id
            }
        }
        impl MultiOneShotHandle {
            pub fn a(
                &self,
                _attributes: (model::X, ResourceGroupAttributes),
            ) -> Result<(), ObserverError> {
                todo!()
            }
            pub fn b(&self, _attributes: model::X) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }

    mod usage {
        use super::*;

        fn example() -> Result<(), Box<dyn std::error::Error>> {
            let one_shot_obs = instrumentation::OneShotObserver {};
            one_shot_obs.one_shot((
                model::OneShot { value: 10 },
                ResourceGroupAttributes {
                    parent_group_id: Uuid::now_v7(),
                },
            ))?;

            let multi_one_shot_obs = instrumentation::MultiOneShotObserver {};
            let handle = multi_one_shot_obs.handle()?;
            handle.a((
                model::X { foo: 10 },
                ResourceGroupAttributes {
                    parent_group_id: Uuid::now_v7(),
                },
            ))?;

            Ok(())
        }
    }
}

// Entities as root resource groups.
// No parent_group_id needed; no ResourceGroupDeclaration injection.
// Multi-event: enum IS the event payload directly (same as non-RG entities).
mod entity_rg_root {
    use super::*;

    mod model {
        use super::*;

        // Single-event root RG.
        // No ResourceGroupAttributes needed for now because that only carries the parent resource id.
        #[derive(Entity, RootResourceGroup)]
        pub struct OneShot {
            pub value: u64,
        }

        pub struct X {
            pub foo: u64,
        }

        // Multi-event root RG.
        // No ResourceGroupAttributes needed for now because that only carries the parent resource id.
        #[derive(Entity, RootResourceGroup)]
        pub enum MultiOneShot {
            A(X),
            B(X),
        }
    }

    mod events {
        // MultiOneShot enum itself is the event payload: Event<MultiOneShot>
    }

    mod instrumentation {

        use super::*;

        pub struct OneShotObserver {}
        impl OneShotObserver {
            // No parent_group_id: root has no parent.
            pub fn one_shot(&self, _attributes: model::OneShot) -> Result<Uuid, ObserverError> {
                todo!()
            }
        }

        pub struct MultiOneShotObserver {}
        impl MultiOneShotObserver {
            pub fn handle(&self) -> Result<MultiOneShotHandle, ObserverError> {
                todo!()
            }
        }
        pub struct MultiOneShotHandle {
            id: Uuid,
        }
        impl EntityDeclaration for model::MultiOneShot {}
        impl EntityHandle for MultiOneShotHandle {
            type DeclarationType = model::MultiOneShot;
            fn id(&self) -> Uuid {
                self.id
            }
        }
        impl MultiOneShotHandle {
            pub fn a(&self, _attributes: model::X) -> Result<(), ObserverError> {
                todo!()
            }
            pub fn b(&self, _attributes: model::X) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }
}

mod fsm_rg {
    use super::*;

    mod model {
        use super::*;
        pub struct X {
            pub foo: u64,
        }

        // For FSMs we need to enforce at least one transition holds the ResourceGroupAttributes.
        #[derive(Fsm, ResourceGroup)]
        #[quent(transitions = {
            entry -> A,
            A -> B,
            B -> exit
        })]
        pub enum Foo {
            A(X, ResourceGroupAttributes),
            B(X),
        }
    }

    mod events {}

    mod instrumentation {

        use super::*;

        // Tag type generated to support the type-state pattern below
        pub struct A;
        pub struct B;

        pub struct FooObserver {
            // holds same stuff as in entity examples
        }

        impl FooObserver {
            pub fn a(
                &self,
                attributes: (model::X, ResourceGroupAttributes), // additional field for the transition to A
            ) -> Result<FooHandle<A>, ObserverError> {
                let id = Uuid::now_v7();
                let _event: Event<Transition<model::Foo>> = Event {
                    id,
                    timestamp: timestamp(),
                    payload: Transition {
                        sequence_number: 0,
                        payload: model::Foo::A(attributes.0, attributes.1),
                    },
                };
                // emitting the event goes here.
                Ok(FooHandle {
                    _phantom: PhantomData,
                    id,
                    next_seq_no: AtomicU16::new(1),
                })
            }
        }

        // A handle for the FSM
        pub struct FooHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            next_seq_no: AtomicU16,
        }
        impl EntityDeclaration for model::Foo {}
        impl<T> EntityHandle for FooHandle<T> {
            type DeclarationType = model::Foo;
            fn id(&self) -> Uuid {
                self.id
            }
        }
        impl FooHandle<A> {
            pub fn b(self) -> Result<FooHandle<B>, ObserverError> {
                todo!()
            }
        }
        impl FooHandle<B> {
            pub fn exit(self) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }
}

mod fsm_rg_root {
    use super::*;

    mod model {
        use super::*;
        pub struct X {
            pub foo: u64,
        }

        #[derive(Fsm, RootResourceGroup)]
        #[quent(transitions = {
            entry -> A,
            A -> B,
            B -> exit
        })]
        pub enum Foo {
            A(X),
            B(X),
        }
    }

    mod events {}

    mod instrumentation {

        use super::*;

        // Tag type generated to support the type-state pattern below
        pub struct A;
        pub struct B;

        pub struct FooObserver {
            // holds same stuff as in entity examples
        }

        impl FooObserver {
            pub fn a(&self, _attributes: model::X) -> Result<FooHandle<A>, ObserverError> {
                todo!()
            }
        }

        // A handle for the FSM
        pub struct FooHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            next_seq_no: AtomicU16,
        }
        impl EntityDeclaration for model::Foo {}
        impl<T> EntityHandle for FooHandle<T> {
            type DeclarationType = model::Foo;
            fn id(&self) -> Uuid {
                self.id
            }
        }
        impl FooHandle<A> {
            pub fn b(self) -> Result<FooHandle<B>, ObserverError> {
                todo!()
            }
        }
        impl FooHandle<B> {
            pub fn exit(self) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }
}

mod invalid {
    use super::*;

    mod model {
        use super::*;

        use crate::v2::resource::Capacity;
        use quent_model_macros::Resource;

        // A resource cannot be a resource group, so this should fail compiling:
        #[derive(Resource, ResourceGroup)]
        pub struct Invalid0 {
            pub bytes: Capacity<u64>,
        }
    }
}
