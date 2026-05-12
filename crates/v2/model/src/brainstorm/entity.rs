// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use super::*;

// An entity that emits one single event only, no attributes (i.e. just records a timestamp).
// The struct itself is the event payload: Event<OneShotEmpty>.
mod one_shot_empty {
    use super::*;

    mod model {
        use super::*;

        // Most trivial entity. Just emits one event without attributes. Thus
        // its only properties are a UUID and a timestamp for its single event.
        #[derive(Entity)]
        pub struct OneShotEmpty;
    }

    mod events {
        // The struct itself is the event payload: Event<OneShotEmpty>
    }

    mod instrumentation {
        use super::*;

        // Each entity gets a dedicated observer in the client application.
        //
        // An observer sinks events and exports them. The observer can e.g.
        // batch events, or even validate entire entities before sending them
        // out, producing error logs etc. when e.g. state transition violation
        // occur. Another example is writing events to a Parquet file per entity
        // without requiring one single schema for the entire model. This makes
        // the schemas simpler, easier to write files, and easier inspect them
        // manually, or with ad-hoc scripts.
        //
        // From this observer, each entity you create can clone the sender into
        // a handle. In this specific case this is not necessary, since we emit
        // exactly one event per entity, so we don't need a handle that keeps
        // entity state (as far as emitting events goes). Examples of a handle
        // are shown in other entities below.
        pub struct OneShotEmptyObserver {}

        impl OneShotEmptyObserver {
            // Returns a result with the uuid of the entity. Since this is a
            // single one shot event entity, it just returns a Uuid.
            pub fn one_shot_empty(&self) -> Result<Uuid, ObserverError> {
                todo!()
            }
        }
    }

    // Future work
    //
    // mod analyzer { ... }
}

// An entity that emits one single event with attributes.
// The struct itself is the event payload: Event<OneShotWithAttribs>.
mod one_shot_with_attribs {
    use super::*;

    pub(crate) mod model {
        use super::*;

        // Single-event entity. The struct fields are the event attributes.
        #[derive(Entity)]
        pub struct OneShotWithAttribs {
            pub foo: u64,
            pub bar: String,
        }
    }

    // Will only show this once as it seems trivial enough
    mod desugared {
        use super::*;
        impl EntityDeclaration for model::OneShotWithAttribs {}
    }

    mod events {
        // The struct itself is the event payload: Event<OneShotWithAttribs>
    }

    mod instrumentation {
        use super::*;
        pub struct OneShotWithAttribsObserver {
            // holds sender
        }
        impl OneShotWithAttribsObserver {
            // Same as OneShotEmpty, does not take &self since there is no
            // state, so we don't need an entity handle yet.
            pub fn one_shot_with_attribs(
                _attributes: super::model::OneShotWithAttribs,
            ) -> Result<Uuid, ObserverError> {
                // emits event
                todo!()
            }
        }
    }

    // Future work
    //
    // mod analyzer { ... }
}

// An entity that emits multiple kinds of events, just once per kind.
// The enum itself is the event payload: Event<MultiOneShot>.
// Enum variants are Once by default; annotate with #[quent(multi)] for multi-emittable events.
mod multi_one_shot {
    use super::*;

    mod model {
        use super::*;

        pub struct X {
            foo: u64,
        }

        pub struct Y {
            bar: String,
            other: EntityRef<one_shot_with_attribs::model::OneShotWithAttribs>,
        }

        #[derive(Entity)]
        pub enum MultiOneShot {
            A(X),
            B(Y),
            C(Y), // same payload type as B, but semantically different event
            D,
        }
    }

    mod events {
        // The enum itself is the event payload: Event<MultiOneShot>
    }

    mod instrumentation {
        use super::*;

        impl EntityDeclaration for model::MultiOneShot {}

        pub struct MultiOneShotObserver {}

        impl MultiOneShotObserver {
            fn handle(&self) -> Result<MultiOneShotHandle, ObserverError> {
                // Returns a new handle, generating a new UUID and cloning the
                // sender, all event flags unset.
                // Could error out if the channel is closed etc.
                todo!()
            }
        }

        pub struct MultiOneShotHandle {
            // - holds its id:
            id: Uuid,
            // - holds an atomic of at least size ceil(log_2(num_events)) as a
            // bitmask for which Once variants have already been emitted. It's
            // unlikely the handle will be attempted to be used by multiple
            // threads to emit the same events, so we don't have to spread this
            // out over num_events bools.
            once_events_emitted: [AtomicU8; 4],
            // - holds sender
            // - doesn't hold a sequence number like FSM because there are no
            // restrictions on emission order, and non-increasing clock reads
            // are rare, but we might consider adding an option to include
            // sequence numbers in the future.
        }

        impl EntityHandle for MultiOneShotHandle {
            type DeclarationType = model::MultiOneShot;
            fn id(&self) -> Uuid {
                self.id
            }
        }

        impl MultiOneShotHandle {
            fn a(&self, _attributes: model::X) -> Result<(), ObserverError> {
                // Once variant: errors out if already emitted, flags in bitmask
                todo!()
            }
            fn b(&self, _attributes: model::Y) -> Result<(), ObserverError> {
                todo!()
            }
            // same payload type as b(), but semantically different event
            fn c(&self, _attributes: model::Y) -> Result<(), ObserverError> {
                todo!()
            }
            fn d(&self) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }

    // Future work
    //
    // mod analyzer { ... }
}

// An entity that emits one kind of event, zero or more times.
// The enum itself is the event payload: Event<OneMultiShot>.
mod one_multi_shot {
    use super::*;

    mod model {
        use super::*;

        pub struct X {
            foo: u64,
        }

        #[derive(Entity)]
        pub enum OneMultiShot {
            #[quent(multi)]
            A(X),
        }
    }

    mod events {
        // The enum itself is the event payload: Event<OneMultiShot>
    }

    mod instrumentation {
        use super::*;

        pub struct OneMultiShotObserver {}

        impl OneMultiShotObserver {
            fn handle(&self) -> Result<OneMultiShotHandle, ObserverError> {
                todo!()
            }
        }

        pub struct OneMultiShotHandle {
            // holds entity UUID
            // holds sender
            // no bitmask: no Once variants
        }

        impl EntityDeclaration for model::OneMultiShot {}
        impl EntityHandle for OneMultiShotHandle {
            type DeclarationType = model::OneMultiShot;

            fn id(&self) -> Uuid {
                todo!()
            }
        }

        impl OneMultiShotHandle {
            fn a(&self, _attributes: super::model::X) -> Result<(), ObserverError> {
                // Could still error out on channel errors etc.
                todo!()
            }
        }
    }
}

// An entity that emits multiple kinds of events, each zero or more times.
// The enum itself is the event payload: Event<MultiMulti>.
mod multi_multi_shot {
    use super::*;

    mod model {
        use super::*;

        pub struct X {
            foo: u64,
        }

        pub struct Y {
            bar: String,
        }

        #[derive(Entity)]
        pub enum MultiMulti {
            #[quent(multi)]
            A(X),
            #[quent(multi)]
            B(X),
            #[quent(multi)]
            C(Y),
        }
    }

    mod events {
        // The enum itself is the event payload: Event<MultiMulti>
    }

    mod instrumentation {
        use super::*;

        pub struct MultiMultiObserver {}

        impl MultiMultiObserver {
            fn handle(&self) -> Result<MultiMultiHandle, ObserverError> {
                todo!()
            }
        }

        pub struct MultiMultiHandle {
            // holds uuid
            // holds sender
            // no bitmask: no Once variants
        }

        impl EntityDeclaration for model::MultiMulti {}
        impl EntityHandle for MultiMultiHandle {
            type DeclarationType = model::MultiMulti;
            fn id(&self) -> Uuid {
                todo!()
            }
        }

        impl MultiMultiHandle {
            fn a(&self, _attributes: super::model::X) -> Result<(), ObserverError> {
                todo!()
            }
            fn b(&self, _attributes: super::model::X) -> Result<(), ObserverError> {
                todo!()
            }
            fn c(&self, _attributes: super::model::Y) -> Result<(), ObserverError> {
                todo!()
            }
        }
    }
}

// An entity with mixed event kinds: some Once (default), some Multi.
// The enum itself is the event payload: Event<Mixed>.
mod mixed {
    use super::*;

    mod model {
        use super::*;

        pub struct X {
            foo: u64,
        }

        pub struct Y {
            bar: String,
        }

        #[derive(Entity)]
        pub enum Mixed {
            A(X), // Once (default)
            #[quent(multi)]
            B(Y), // Multi
        }
    }

    mod events {
        // The enum itself is the event payload: Event<Mixed>
    }

    mod instrumentation {
        use super::*;

        pub struct MixedObserver {}

        impl MixedObserver {
            fn handle(&self) -> Result<MixedHandle, ObserverError> {
                todo!()
            }
        }

        pub struct MixedHandle {
            // holds entity uuid
            // holds sender
            // holds bitmask for Once variants only (in this case only A)
        }

        impl EntityDeclaration for model::Mixed {}
        impl EntityHandle for MixedHandle {
            type DeclarationType = model::Mixed;
            fn id(&self) -> Uuid {
                todo!()
            }
        }

        impl MixedHandle {
            pub fn a(&self, _attributes: super::model::X) -> Result<(), ObserverError> {
                // Once variant: errors out if already emitted
                // Can also still error out on channel errors etc.
                todo!()
            }

            pub fn b(&self, _attributes: super::model::Y) -> Result<(), ObserverError> {
                // Could still error out on channel errors etc.
                todo!()
            }
        }
    }

    // Future work
    //
    // mod analyzer { ... }
}

// Invalid things that should produce compilation errors.
mod invalid {
    use super::*;

    mod model {
        use super::*;

        // An entity enum must have at least one variant.
        #[derive(Entity)]
        pub enum Invalid {}
    }
}
