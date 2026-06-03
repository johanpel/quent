// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Constraint marking an entity reference as tree-forming.

/// Constraint to express a tree connecting all entities within a graph where
/// vertice represent entities and entity references representing edges.
///
/// This constraint can be used for arbitrary purposes. Its canonical purpose
/// is to provide some "preferred" way of traversing entities and their events
/// from a single starting point (the root entity), e.g. such that some user
/// interface can help a human traverse the trace in this preferred way.
///
/// References annotated with this constraint are typically used (but not
/// limited) to express:
/// - Causal relations (e.g. entity Y was produced by entity X)
/// - Scopes (e.g. entity Y is part of X)
/// - Ownership relations (e.g. entity Y is owned by entity X)
///
/// In order for instrumentation libraries to provide strong guarantees
/// (typically compile-time) that this constraint is met, the tree must be fully
/// defined at "schema-time". Therefore, type-erased entity references cannot
/// carry an annotation with this constraint, as this would allow forming entity
/// graphs that are not trees (i.e. multiple instances of an entity of type A
/// would be able to emit events that refer to both an entity of type B and of
/// type C). For this reason, this constraint depends on the constraint provided
/// by the [`quent_ref_target`] crate.
///
/// ## Requirements
///
/// 1. The schema has exactly one entity (a.k.a. the root entity) that does not
///    carry an entity reference annotated with this constraint in any of its
///    events.
/// 2. Every non-root entity has at least one event carrying an entity
///    reference annotated with this constraint to declare it refers to exactly
///    one type of parent entity in the tree (a.k.a. a parent entity reference).
/// 3. Every parent entity reference must be target-constrained (carry a
///    [`quent_ref_target`] annotation). A type-erased reference may not carry
///    this constraint (implied by requirement 2).
/// 4. There is exactly one path from every non-root entity type to the root
///    entity type through parent entity references.
///
/// ## Note on possible parent ambiguity (req. 2)
///
/// Parent ambiguity at run-time can exist through multiple parent-declaring
/// events, which is allowed by requirement 2.
///
/// Since client code can have branching behavior where certain events are
/// conditionally emitted, this constraint permits the parent reference to be
/// placed (once) on any number of events, even though logically speaking, it
/// can only have one parent, and it would ideally emit its parent reference
/// exactly once. It is the responsibility of the client code to ensure it
/// produces an unambiguous event stream with regards to this tree-forming
/// constraint.
///
/// This constraint intentionally defers any potential resolution to the problem
/// of clients producing ambiguous event streams to schema producer / consumer
/// implementations.
///
/// For example, a modeling API or DSL _could_ decide to enforce FSM entities to
/// always declare their parent in the initial state. An instrumentation library
/// _could_ error on emitting a second parent-declaring event if it changes the
/// reference value. An analysis library _could_ produce an error when an event
/// stream is ingested exhibiting this ambiguity.
pub struct RefTreeConstraint;
