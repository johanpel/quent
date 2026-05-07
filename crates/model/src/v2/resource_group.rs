// An entity can be a resource group, which means that at least one of its
// events needs to carry certain attributes.
//
// 0. How to make this a resource group?
//
// a. #[derive(ResourceGroup)] ?
// b. #[quent(resource_group)] ? may be useful because see below
// c. ...?
//
// 1. How do we mark that event?
//
// a. Make the field use some OnceWithResourceGroupAttributes<T> ?
// b. With a field annotation? #[quent(resource_group)] ?
// c. With a struct annotation? #[quent(resource_group(a))] ?
// d. ... ?
//
// 2. if an entity is the root resource group, it does not require a parent.
// How to convey that?
//
// a. OnceWithRootResourceGroupAttributes<T> ?  -> ugly and potential state explosion
// b. Event field annotation #[quent(resource_group(root))] ?
// c. #[quent(resource_group(a, root))] ?
// d. ... ?
//
// Should multi events be able to carry the resource group attributes?
//
// Choices made below:
// 0. A
// 1. b.
// 2. b, but with a struct-level annotation

use crate::v2::{
    entity::{EntityHandle, Event, ObserverError, Once},
    fsm::Transition,
};
use quent_model_macros::{Entity, Fsm, ResourceGroup};
use quent_time::timestamp;

use std::{marker::PhantomData, sync::atomic::AtomicU16};

use uuid::Uuid;

pub struct ResourceGroupDeclaration {
    pub parent_group_id: Uuid,
}

// An entities as resource group
mod entity_rg {
    use super::*;

    mod model {
        use super::*;

        #[derive(Entity, ResourceGroup)]
        pub struct OneShot {
            pub value: u64,
        }

        pub struct X {
            pub foo: u64,
        }

        #[derive(Entity, ResourceGroup)]
        pub struct MultiOneShot {
            // marks this event as the one that will carry the resource group properties
            #[quent(resource_group(declare))]
            pub a: Once<X>,
            pub b: Once<X>,
        }
    }

    mod events {
        use super::*;

        pub enum MultiOneShotEvent {
            A(model::X, ResourceGroupDeclaration),
            B(model::X),
        }
    }

    mod instrumentation {
        use crate::v2::entity::EntityHandle;

        use super::*;

        pub struct OneShotObserver {}
        impl OneShotObserver {
            pub fn one_shot(
                &self,
                _attributes: model::OneShot,
                _parent_group_id: Uuid, // additional field due to ResourceGroup
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
        impl EntityHandle for MultiOneShotHandle {
            fn id(&self) -> Uuid {
                self.id
            }
        }
        impl MultiOneShotHandle {
            pub fn a(
                &self,
                _attributes: model::X,
                _parent_group_id: Uuid,
            ) -> Result<(), ObserverError> {
                todo!()
            }
            pub fn b(&self, _attributes: model::X) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }
}

// Entities as root resource groups
mod entity_rg_root {
    use super::*;

    mod model {
        use super::*;

        #[derive(Entity, ResourceGroup)]
        #[quent(resource_group(root))] // annotates this is a root resource group
        pub struct OneShot {
            pub value: u64,
        }

        pub struct X {
            pub foo: u64,
        }

        #[derive(Entity, ResourceGroup)]
        #[quent(resource_group(root))] // annotates this is a root resource group
        pub struct MultiOneShot {
            // Since this is a root resource, no event needs to carry the parent
            // group id, so we don't need to annotate any event like in the non-root
            // case.
            pub a: Once<X>,
            pub b: Once<X>,
        }
    }

    mod instrumentation {

        use super::*;

        pub struct OneShotObserver {}
        impl OneShotObserver {
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
        impl EntityHandle for MultiOneShotHandle {
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

        // TODO: perhaps always have resource group attributes included in the entry transition? this will prevent redeclaration too.
        #[derive(Fsm, ResourceGroup)]
        #[quent(transitions = {
            entry -> A,
            A -> B,
            B -> exit
        })]
        pub enum Foo {
            #[quent(resource_group(declare))]
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
            pub fn a(
                &self,
                attributes: model::X,
                parent_group_id: Uuid, // additional field for the transition to A
            ) -> Result<FooHandle<A>, ObserverError> {
                let id = Uuid::now_v7();
                // payload is now a tuple with the resource group declaration added
                let _event: Event<Transition<(model::Foo, ResourceGroupDeclaration)>> = Event {
                    id,
                    timestamp: timestamp(),
                    payload: Transition {
                        sequence_number: 0,
                        payload: (
                            model::Foo::A(attributes),
                            ResourceGroupDeclaration { parent_group_id },
                        ),
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
        impl<T> EntityHandle for FooHandle<T> {
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
