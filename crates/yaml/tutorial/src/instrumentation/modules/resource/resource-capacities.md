# Resource capacities

A resource can expose measured capacities. `occupancy` describes a quantity
held over the usage span, such as bytes of memory held while a task runs.

## YAML model

```yaml
{{#include ../../../../../examples/resource-capacity/model.yaml}}
```

Declaring the `bytes` capacity generates a `MemoryUsage` record with a `bytes`
field.

## Instrumentation API

The task's reference to `Memory` carries the quantity it claims.

```rust
{{#include ../../../../../examples/resource-capacity/src/main.rs}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Key point</strong>
    <p>A capacity resource records the quantity of a resource used by an entity.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="10">
  <h2>Check yourself</h2>
  <fieldset data-answer="b" data-explanation="Occupancy is a quantity held over a usage span, such as allocated bytes while running.">
    <legend>What does an <code>occupancy</code> capacity measure?</legend>
    <label><input type="radio" name="q10a" value="a"> A one-time event count</label>
    <label><input type="radio" name="q10a" value="b"> A quantity held during a usage span</label>
    <label><input type="radio" name="q10a" value="c"> A hierarchy depth</label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="c" data-explanation="MemoryUsage is generated from Memory's capacities and carries the claimed bytes.">
    <legend>Where is the task's claimed byte quantity stored?</legend>
    <label><input type="radio" name="q10b" value="a"> <code>ResourceCapacityContext</code></label>
    <label><input type="radio" name="q10b" value="b"> The task UUID</label>
    <label><input type="radio" name="q10b" value="c"> <code>MemoryUsage</code></label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
