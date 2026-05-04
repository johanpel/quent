use crate::v2::entity::{EntityHandle, ObserverError};
use quent_model_macros::Fsm;
use std::marker::PhantomData;
use uuid::Uuid;

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

// This shouldn't compile. States need a name and an attributes type,
// which none of these definitions can provide:
mod invalid {
    use super::*;

    mod model {
        use super::*;

        #[derive(Fsm)]
        pub enum Invalid {}
    }
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
        // No generated types here, X is the only state
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

        impl<T> EntityHandle for SingleEmptyHandle<T> {
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
            pub fn a(&self, _attributes: X) -> Result<SingleAttribsHandle<A>, ObserverError> {
                todo!()
            }
        }

        // A handle for the FSM
        pub struct SingleAttribsHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
        }

        impl<T> EntityHandle for SingleAttribsHandle<T> {
            fn id(&self) -> Uuid {
                self.id
            }
        }

        impl SingleAttribsHandle<A> {
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
        }

        impl<T> EntityHandle for MultiSeqHandle<T> {
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
        }

        impl<T> EntityHandle for SoloLoopHandle<T> {
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
mod fanout {
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
        }

        impl<T> EntityHandle for FanoutHandle<T> {
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

mod fanin {
    use super::*;

    mod model {
        use super::*;

        #[derive(Fsm)]
        #[quent(transitions={entry->A, A->{B,C}, {B, C}->D, D->exit})]
        pub enum Fanin {
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

        pub struct FaninObserver {
            // holds same stuff as in entity examples
        }

        impl FaninObserver {
            // Initial state transition produces a handle with an API following
            // the type-state pattern
            pub fn a(&self) -> Result<FaninHandle<A>, ObserverError> {
                todo!()
            }
        }

        // A handle for the FSM
        pub struct FaninHandle<T> {
            _phantom: PhantomData<T>,
            id: Uuid,
        }

        impl<T> EntityHandle for FaninHandle<T> {
            fn id(&self) -> Uuid {
                self.id
            }
        }

        impl FaninHandle<A> {
            pub fn b(self, _attributes: X) -> Result<FaninHandle<B>, ObserverError> {
                todo!()
            }
            pub fn c(self) -> Result<FaninHandle<C>, ObserverError> {
                todo!()
            }
        }

        impl FaninHandle<B> {
            pub fn d(self, _attributes: Y) -> Result<FaninHandle<D>, ObserverError> {
                todo!()
            }
        }

        impl FaninHandle<C> {
            pub fn d(self, _attributes: Y) -> Result<FaninHandle<D>, ObserverError> {
                todo!()
            }
        }

        impl FaninHandle<D> {
            pub fn exit(self) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }

    mod usage {
        use super::*;

        fn example() -> Result<(), Box<dyn std::error::Error>> {
            let obs = instrumentation::FaninObserver {};

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

mod full {
    use super::*;

    mod model {
        use super::*;

        #[derive(Fsm)]
        #[quent(transitions={entry->A, A->{B,C}, B->B, {B, C}->D, D->exit})]
        pub enum Multi {
            A(()),
            B(Y),
            C,
            D(X),
        }
    }
}
