// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use super::*;

// Considerations:
//
// While theoretically we could first generate a #[derive(Entity)] from a
// #[derive(Fsm)], it would be harder to generate FSM entity instrumentation
// APIs with the type-state pattern from there, so #[derive(Fsm)] will not take
// that approach, but we should figure out what functionaltiy between those two
// derives overlaps and deduplicate any logic.
//
// To compile a set of states an Fsm can be in, I've considered declaring a
// struct where each field is the state name and the field type are the
// attribute types. However, I find the enum style more compelling since an FSM
// is always in exactly one state at any moment, which naturally translates to a
// sum type.
//
// Since all transitions are compile-time validated for correctness, as far as
// possible sequences defined by the FSMs topology is allowed, any errors the
// transition event calls return are going to be sender channel related. There
// is no recovery from these errors, so FSM handles are dropped. Future work can
// consider returning the handle in some erroneous state.

// Arbitrary attribute types
pub struct X {
    pub foo: u64,
}
pub struct Y {
    pub bar: String,
}

// FSM with just one state without attributes
mod single_empty {
    use super::*;

    mod model {
        use super::*;

        // No quent(transition) macro attribute. Since there is one state, it
        // must be the entry state and it is a final state.
        #[derive(Fsm)]
        pub enum SingleEmpty {
            A,
        }
    }

    mod events {
        // No generated types here, A is the only state
    }

    mod instrumentation {
        use super::*;

        // Tag type generated to support the type-state pattern below
        pub struct A;

        pub struct SingleEmptyObserver {
            // holds same stuff as in entity examples
        }

        impl SingleEmptyObserver {
            // Initial state transition produces a handle with an API following
            // the type-state pattern
            pub fn a(&self) -> Result<SingleEmptyHandle<A>, ObserverError> {
                todo!()
            }
        }

        // A handle for the FSM
        pub struct SingleEmptyHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
        }

        impl EntityDeclaration for model::SingleEmpty {}
        impl<T> EntityHandle for SingleEmptyHandle<T> {
            type DeclarationType = model::SingleEmpty;
            fn id(&self) -> Uuid {
                self.id
            }
        }

        impl SingleEmptyHandle<A> {
            // Exit consumes and drops the handle
            pub fn exit(self) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }

    mod analyzer {
        // Whatever backs this model, it needs to implement Fsm.
        // TODO: this is a circular dep right now which won't be the case once it's generated.
        // pub trait SingleAttribsModel: quent_analyzer::fsm::Fsm {
        //     // Only one state that we can enter only once and exit only once.
        //     fn a() -> Option<FsmStateRef<'a, Self, Self::TransitionType>>;
        // }
    }

    mod usage {
        use super::*;

        fn example() -> Result<(), Box<dyn std::error::Error>> {
            let obs = instrumentation::SingleEmptyObserver {};
            let handle = obs.a()?;
            // Need to grab ID here:
            println!("id: {}", handle.id());

            handle.exit()?;

            // or
            obs.a()?.exit()?;

            // In future work we can consider not dropping the handle yet after
            // exit such that the id and other properties could be read from the
            // handle (e.g. which state was emitted for multi state things
            // below)

            Ok(())
        }
    }
}

// FSM with just one state with atttributes
mod single_attribs {
    use super::*;

    mod model {
        use super::*;

        // Same here, no quent(transition) macro attribute. The only state is A,
        // which is implicitly the entry state and a final state.
        #[derive(Fsm)]
        pub enum SingleAttribs {
            A(X),
        }
    }

    mod events {
        // No generated types here, SingleAttribs is the event payload type already
    }

    mod instrumentation {
        use super::*;

        // Tag type generated to support the type-state pattern below
        pub struct A;

        pub struct SingleAttribsObserver {
            // holds same stuff as in entity examples
        }

        impl SingleAttribsObserver {
            // Initial state transition produces a handle with an API following
            // the type-state pattern
            pub fn a(&self, attributes: X) -> Result<SingleAttribsHandle<A>, ObserverError> {
                let id = Uuid::now_v7();
                let _event: Event<Transition<model::SingleAttribs>> = Event {
                    id,
                    timestamp: timestamp(),
                    data: Transition {
                        sequence_number: 0,
                        payload: model::SingleAttribs::A(attributes),
                    },
                };

                // emitting the event goes here.

                Ok(SingleAttribsHandle {
                    _phantom: PhantomData,
                    id,
                    next_seq_no: AtomicU16::new(1),
                })
            }
        }

        // A handle for the FSM
        pub struct SingleAttribsHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            next_seq_no: AtomicU16,
        }

        impl EntityDeclaration for model::SingleAttribs {}
        impl<T> EntityHandle for SingleAttribsHandle<T> {
            type DeclarationType = model::SingleAttribs;
            fn id(&self) -> Uuid {
                self.id
            }
        }

        impl SingleAttribsHandle<A> {
            pub fn exit(self) -> Result<(), ObserverError> {
                let _event: Event<Transition<()>> = Event {
                    id: self.id,
                    timestamp: timestamp(),
                    data: Transition {
                        sequence_number: self
                            .next_seq_no
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                        payload: (),
                    },
                };
                // emitting the event goes here.
                Ok(())
            }
        }
    }

    mod analyzer {
        // Whatever backs this model, it needs to implement Fsm.
        // TODO: this is a circular dep right now which won't be the case once it's generated.
        // pub trait SingleAttribsModel: quent_analyzer::fsm::Fsm {
        //     // Only one state that we can enter only once and exit only once.
        //     fn a() -> Option<FsmStateRef<'a, Self, Self::TransitionType>>;
        // }
    }

    mod usage {
        use super::*;

        fn example() -> Result<(), Box<dyn std::error::Error>> {
            let obs = instrumentation::SingleAttribsObserver {};
            let handle = obs.a(X { foo: 10 })?;
            handle.exit()?;
            Ok(())
        }
    }
}

// FSM with multiple states through which it must go in a sequence
mod multi_seq {
    use super::*;

    mod model {
        use super::*;

        #[derive(Fsm)]
        #[quent(transitions={
            entry->A,
            A->B,
            B->C,
            C->exit
        })]
        pub enum MultiSeq {
            A(X),
            B(Y),
            C(Y), // same attributes type, but semantically different state
        }

        // Note: we could provide syntactic sugar later to simplify this to:
        // #[quent(transitions={entry->A->B->C->exit})]
    }

    mod events {
        // No new event type, because the FSM enum is already the event payload
        // type
    }

    mod instrumentation {
        use super::*;

        // Tag types generated to support the type-state pattern below
        pub struct A;
        pub struct B;
        pub struct C;

        pub struct MultiSeqObserver {
            // holds same stuff as in entity examples
        }

        impl MultiSeqObserver {
            // Initial state transition produces a handle with an API following
            // the type-state pattern
            pub fn a(&self, _attributes: X) -> Result<MultiSeqHandle<A>, ObserverError> {
                todo!()
            }
        }

        // A handle for the FSM
        pub struct MultiSeqHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            next_seq_no: AtomicU16,
        }

        impl EntityDeclaration for model::MultiSeq {}
        impl<T> EntityHandle for MultiSeqHandle<T> {
            type DeclarationType = model::MultiSeq;
            fn id(&self) -> Uuid {
                self.id
            }
        }

        impl MultiSeqHandle<A> {
            pub fn b(self, _attributes: Y) -> Result<MultiSeqHandle<B>, ObserverError> {
                todo!()
            }
        }

        impl MultiSeqHandle<B> {
            pub fn c(self, _attributes: Y) -> Result<MultiSeqHandle<C>, ObserverError> {
                todo!()
            }
        }

        impl MultiSeqHandle<C> {
            pub fn exit(self) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }

    mod analyzer {
        // pub trait MultiSeqModel: quent_analyzer::fsm::Fsm {
        //     // All states can only be entered once.
        //     fn a() -> Option<FsmStateRef<'a, Self, Self::TransitionType>>;
        //     fn b() -> Option<FsmStateRef<'a, Self, Self::TransitionType>>;
        //     fn c() -> Option<FsmStateRef<'a, Self, Self::TransitionType>>;
        // }
    }

    mod usage {
        use super::*;

        fn example() -> Result<(), Box<dyn std::error::Error>> {
            let obs = super::instrumentation::MultiSeqObserver {};

            let handle = obs.a(X { foo: 1337 })?;

            // We can get the ID generated when the handle was constructed.
            println!("{}", handle.id());

            // handle.c() - doesn't compile

            let handle = handle.b(Y { bar: "hi".into() })?;

            handle
                .c(Y { bar: "bye".into() })? // we can chain if we want
                .exit()?; // but exit drops the handle
            Ok(())
        }
    }
}

// FSM with a single state that can transition into itself
mod solo_loop {
    use super::*;

    mod model {
        use super::*;

        #[derive(Fsm)]
        #[quent(transitions = {entry->A, A->A, A->exit})]
        pub enum SoloLoop {
            A(X),
        }
    }

    mod events {}

    mod instrumentation {
        use super::*;

        // Tag types generated to support the type-state pattern below
        pub struct A;

        pub struct SoloLoopObserver {
            // holds same stuff as in entity examples
        }

        impl SoloLoopObserver {
            // Initial state transition produces a handle with an API following
            // the type-state pattern
            pub fn a(&self, _attributes: X) -> Result<SoloLoopHandle<A>, ObserverError> {
                todo!()
            }
        }

        // A handle for the FSM
        pub struct SoloLoopHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            next_seq_no: AtomicU16,
        }

        impl EntityDeclaration for model::SoloLoop {}
        impl<T> EntityHandle for SoloLoopHandle<T> {
            type DeclarationType = model::SoloLoop;
            fn id(&self) -> Uuid {
                self.id
            }
        }

        impl SoloLoopHandle<A> {
            pub fn a(self, _attributes: X) -> Result<SoloLoopHandle<A>, ObserverError> {
                todo!()
            }
            pub fn exit(self) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }

    mod analyzer {
        // pub trait SoloLoopModel: quent_analyzer::fsm::Fsm {
        //     // Returns every instance of A.
        //     fn a() -> impl Iterator<Item = FsmStateRef<'a, Self, Self::TransitionType>>>;
        // }
    }

    mod usage {
        use super::*;

        fn example() -> Result<(), Box<dyn std::error::Error>> {
            let obs = instrumentation::SoloLoopObserver {};

            let handle = obs.a(X { foo: 1 })?;
            let handle = handle.a(X { foo: 2 })?;
            handle.a(X { foo: 3 })?.exit()?;

            Ok(())
        }
    }
}

// FSM with a state with multiple next states
mod fan_out {
    use super::*;

    mod model {
        use super::*;

        #[derive(Fsm)]
        #[quent(transitions={entry->A, A->{B, C}, B->D, C->D, D->exit})]
        pub enum Fanout {
            A(X),
            B,
            C(Y),
            D,
        }
    }

    mod events {}

    mod instrumentation {
        use super::*;

        // Tag types generated to support the type-state pattern below
        pub struct A;
        pub struct B;
        pub struct C;
        pub struct D;

        pub struct FanoutObserver {
            // holds same stuff as in entity examples
        }

        impl FanoutObserver {
            // Initial state transition produces a handle with an API following
            // the type-state pattern
            pub fn a(&self, _attributes: X) -> Result<FanoutHandle<A>, ObserverError> {
                todo!()
            }
        }

        // A handle for the FSM
        pub struct FanoutHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            next_seq_no: AtomicU16,
        }

        impl EntityDeclaration for model::Fanout {}
        impl<T> EntityHandle for FanoutHandle<T> {
            type DeclarationType = model::Fanout;
            fn id(&self) -> Uuid {
                self.id
            }
        }

        impl FanoutHandle<A> {
            pub fn b(self) -> Result<FanoutHandle<B>, ObserverError> {
                todo!()
            }
            pub fn c(self, _attributes: Y) -> Result<FanoutHandle<C>, ObserverError> {
                todo!()
            }
        }

        impl FanoutHandle<B> {
            pub fn d(self) -> Result<FanoutHandle<D>, ObserverError> {
                todo!()
            }
        }

        impl FanoutHandle<C> {
            pub fn d(self) -> Result<FanoutHandle<D>, ObserverError> {
                todo!()
            }
        }

        impl FanoutHandle<D> {
            pub fn exit(self) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }

    mod usage {
        use super::*;

        fn example() -> Result<(), Box<dyn std::error::Error>> {
            let obs = instrumentation::FanoutObserver {};

            obs.a(X { foo: 10 })?.b()?.d()?.exit()?;

            // or
            obs.a(X { foo: 10 })?
                .c(Y {
                    bar: "bar".to_string(),
                })?
                .d()?
                .exit()?;

            Ok(())
        }
    }
}

// FSM with multiple states transitioning into one next state
mod fan_in {
    use super::*;

    mod model {
        use super::*;

        #[derive(Fsm)]
        #[quent(transitions={entry->A, A->{B,C}, {B, C}->D, D->exit})]
        pub enum FanIn {
            A(()),
            B(X),
            C,
            D(Y),
        }
    }

    mod events {}

    mod instrumentation {
        use super::*;

        // Tag types generated to support the type-state pattern below
        pub struct A;
        pub struct B;
        pub struct C;
        pub struct D;

        pub struct FanInObserver {
            // holds same stuff as in entity examples
        }

        impl FanInObserver {
            // Initial state transition produces a handle with an API following
            // the type-state pattern
            pub fn a(&self) -> Result<FanInHandle<A>, ObserverError> {
                todo!()
            }
        }

        // A handle for the FSM
        pub struct FanInHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            next_seq_no: AtomicU16,
        }

        impl EntityDeclaration for model::FanIn {}
        impl<T> EntityHandle for FanInHandle<T> {
            type DeclarationType = model::FanIn;
            fn id(&self) -> Uuid {
                self.id
            }
        }

        impl FanInHandle<A> {
            pub fn b(self, _attributes: X) -> Result<FanInHandle<B>, ObserverError> {
                todo!()
            }
            pub fn c(self) -> Result<FanInHandle<C>, ObserverError> {
                todo!()
            }
        }

        impl FanInHandle<B> {
            pub fn d(self, _attributes: Y) -> Result<FanInHandle<D>, ObserverError> {
                todo!()
            }
        }

        impl FanInHandle<C> {
            pub fn d(self, _attributes: Y) -> Result<FanInHandle<D>, ObserverError> {
                todo!()
            }
        }

        impl FanInHandle<D> {
            pub fn exit(self) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }

    mod usage {
        use super::*;

        fn example() -> Result<(), Box<dyn std::error::Error>> {
            let obs = instrumentation::FanInObserver {};

            obs.a()?
                .b(X { foo: 10 })?
                .d(Y {
                    bar: "hi".to_string(),
                })?
                .exit()?;

            // or
            obs.a()?
                .c()?
                .d(Y {
                    bar: "bye".to_string(),
                })?
                .exit()?;

            Ok(())
        }
    }
}

// Full example with fanin, fanout, loop
mod full {
    use super::*;

    mod model {
        use super::*;

        #[derive(Fsm)]
        #[quent(transitions={entry->A, A->{B,C}, B->B, {B, C}->D, D->exit})]
        pub enum Full {
            A,
            B(Y),
            C,
            D(X),
        }
    }

    mod events {}

    mod instrumentation {
        use super::*;

        pub struct A;
        pub struct B;
        pub struct C;
        pub struct D;

        pub struct FullObserver {}
        impl FullObserver {
            pub fn a(&self) -> Result<FullHandle<A>, ObserverError> {
                todo!()
            }
        }

        pub struct FullHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
            next_seq_no: AtomicU16,
        }
        impl FullHandle<A> {
            pub fn b(self, _attributes: Y) -> Result<FullHandle<B>, ObserverError> {
                todo!()
            }
            pub fn c(self) -> Result<FullHandle<C>, ObserverError> {
                todo!()
            }
        }
        impl FullHandle<B> {
            pub fn b(self, _attributes: Y) -> Result<FullHandle<B>, ObserverError> {
                todo!()
            }
            pub fn d(self, _attributes: X) -> Result<FullHandle<D>, ObserverError> {
                todo!()
            }
        }
        impl FullHandle<C> {
            pub fn d(self, _attributes: X) -> Result<FullHandle<D>, ObserverError> {
                todo!()
            }
        }
        impl FullHandle<D> {
            pub fn exit(self) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }
}

// Fsm declarations that shouldn't compile.
mod invalid {
    use super::*;

    mod model {
        use super::*;

        // Shouldn't compile, because states need a name and an attributes type.
        #[derive(Fsm)]
        pub enum Invalid0 {}

        // Shouldn't compile, no entry state
        #[derive(Fsm)]
        #[quent(transitions = {
            A -> B,
            B -> exit
        })]
        pub enum Invalid1 {
            A,
            B,
        }

        // Shouldn't compile, no exit state
        #[derive(Fsm)]
        #[quent(transitions = {
            entry -> A,
            A -> B,
        })]
        pub enum Invalid2 {
            A,
            B,
        }

        // Shouldn't compile, cannot enter into exit
        #[derive(Fsm)]
        #[quent(transitions = {
            entry -> exit,
            A -> B
        })]
        pub enum Invalid3 {
            A,
            B,
        }

        // Shouldn't compile, has unreachable states
        #[derive(Fsm)]
        #[quent(transitions = {
            entry -> A,
            A -> exit
            B -> A
        })]
        pub enum Invalid4 {
            A,
            B,
        }
    }
}
