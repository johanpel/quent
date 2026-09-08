# Resource

The Resource semantic module describes what an application provides and what
its work consumes:

- A **resource** is an entity that can be claimed, such as a thread, worker, or
  memory pool.
- A **capacity** is a named quantity provided by a resource.
  - An **occupancy** is a quantity held throughout a usage. For example, a task
    might occupy 256 bytes of a memory pool until it leaves its current state.
  - A **rate** records a total quantity processed during a usage. For example,
    a transfer might process 1,024 bytes. Dividing that value by the usage
    duration gives the observed transfer rate.
  - A resource with no named capacities is a **unit resource**. Each usage
    claims the entire resource instance.
- A **usage** is a claim on a specific resource instance. It identifies the
  resource and records how much of each capacity is claimed.
- A **bound** is a reported upper limit for a capacity. Bounds belong to the
  resource and may be updated by its events.

Only entities modeled as FSMs can use resources.

## Why can only FSMs use resources?

Entering a state starts the usages declared by that state, and leaving it ends
them. A final state cannot start a usage. The validated FSM topology therefore
gives every usage a modeled way to end.

This is a schema-level guarantee, not a runtime guarantee. The instrumenting
code remains responsible for emitting a valid transition out of a state that
uses resources. If it does not, the event stream contains a logically unended
usage.
