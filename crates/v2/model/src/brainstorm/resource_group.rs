// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use super::*;

// Two types of entities as a non-root resource group and one resource root
// entity acting as resource tree root.
mod entity_rg {
    use super::*;

    mod model {
        use super::*;

        // This is a root resource.
        #[derive(Entity, RootResourceGroup)]
        pub struct Root;

        // Single-event entity RG.
        #[derive(Entity, ResourceGroup)]
        pub struct OneShot {
            pub some_attribute: u64,
            // Arbitrarily named field carrying the type-safe resource group
            // parent reference. Since this is a one shot entity and a non-root
            // resource group requires an event conveying its parent, at least
            // one struct field must contain this type of reference.
            pub parent: EntityRef<Root, RgParentRef>,
        }

        pub struct X {
            pub foo: u64,
        }

        // Multi-event entity RG.
        //
        // Again, it is required to have at least one event declare what the
        // parent resource group is. This can't be done in the X attribute set,
        // because the ResourceGroup macro can't look at that type definition.
        // Instead, the macro will require there to exist exactly one inline
        // struct with a field with an EntityRef of kind ResourceGroupParentRef
        #[derive(Entity, ResourceGroup)]
        pub enum MultiOneShot {
            A {
                x: X,
                parent: EntityRef<OneShot, RgParentRef>,
            },
            B(X),
        }

        // Multi-event entity RG with a parent that can be any type of resource group.
        //
        // The macro requires at least one event to declare what the parent
        // resource group is.
        #[derive(Entity, ResourceGroup)]
        pub enum WithAny {
            A {
                x: X,
                parent: EntityRef<AnyRg, RgParentRef>,
            },
            B(X),
        }
    }

    mod events {}

    mod instrumentation {
        use super::*;

        impl EntityDeclaration for model::Root {}
        impl ResourceGroupDeclaration for model::Root {}
        pub struct RootObserver {}
        impl RootObserver {
            pub fn root(&self) -> Result<EntityRef<model::Root>, ObserverError> {
                let id = Uuid::now_v7();
                // emit event goes here.
                Ok(EntityRef {
                    _entity: PhantomData,
                    _ref_kind: PhantomData,
                    id,
                })
            }
        }

        impl EntityDeclaration for model::OneShot {}
        impl ResourceGroupDeclaration for model::OneShot {}
        pub struct OneShotObserver {}
        impl OneShotObserver {
            // The parent is now part of model::OneShot itself, so the observer
            // takes the struct directly — no extra tuple, no extra argument.
            pub fn one_shot(
                &self,
                _attributes: model::OneShot,
            ) -> Result<EntityRef<model::OneShot>, ObserverError> {
                let id = Uuid::now_v7();
                // emit event goes here.
                Ok(EntityRef {
                    _entity: PhantomData,
                    _ref_kind: PhantomData,
                    id,
                })
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
            // Named arguments mirror the named fields of variant A.
            pub fn a(
                &self,
                _x: model::X,
                _parent: EntityRef<model::OneShot, RgParentRef>,
            ) -> Result<(), ObserverError> {
                todo!()
            }
            pub fn b(&self, _attributes: model::X) -> Result<(), ObserverError> {
                todo!()
            }
        }

        pub struct WithAnyObserver {}
        impl WithAnyObserver {
            pub fn handle(&self) -> Result<WithAnyHandle, ObserverError> {
                todo!()
            }
        }
        pub struct WithAnyHandle {
            id: Uuid,
        }
        impl EntityDeclaration for model::WithAny {}
        impl ResourceGroupDeclaration for model::WithAny {}
        impl EntityHandle for WithAnyHandle {
            type DeclarationType = model::WithAny;
            fn id(&self) -> Uuid {
                self.id
            }
        }
        impl WithAnyHandle {
            // Named arguments mirror the named fields of variant A.
            pub fn a(
                &self,
                _x: model::X,
                _parent: EntityRef<AnyRg, RgParentRef>,
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
            let root_obs = instrumentation::RootObserver {};

            let root = root_obs.root()?;

            let one_shot_obs = instrumentation::OneShotObserver {};
            let one_shot = one_shot_obs.one_shot(model::OneShot {
                some_attribute: 10,
                // into to convert it from a regular entity ref to a ref acting
                // as a parent resource group ref
                parent: root.into(),
            })?;

            let multi_obs = instrumentation::MultiOneShotObserver {};
            let multi_handle = multi_obs.handle()?;
            multi_handle.a(model::X { foo: 10 }, one_shot.into())?;

            let with_any_obs = instrumentation::WithAnyObserver {};
            let with_any_handle = with_any_obs.handle()?;
            with_any_handle.a(model::X { foo: 10 }, one_shot.into_erased())?;

            Ok(())
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

        // For FSMs we need to enforce that exactly one transition variant
        // declares the parent. As with multi-event entity RGs, a named struct
        // variant keeps the field name (`parent`) visible.
        #[derive(Fsm, ResourceGroup)]
        #[quent(transitions = {
            entry -> A,
            A -> B,
            B -> exit
        })]
        pub enum Foo {
            A {
                x: X,
                parent: EntityRef<AnyRg, RgParentRef>,
            },
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
                x: model::X,
                parent: EntityRef<AnyRg, RgParentRef>,
            ) -> Result<FooHandle<A>, ObserverError> {
                let id = Uuid::now_v7();
                let _event: Event<Transition<model::Foo>> = Event {
                    id,
                    timestamp: timestamp(),
                    data: Transition {
                        sequence_number: 0,
                        payload: model::Foo::A { x, parent },
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

        // A resource cannot be a resource group, so this should fail compiling:
        #[derive(Resource, ResourceGroup)]
        pub struct Invalid0 {
            pub bytes: Capacity<u64>,
        }
    }
}
