# 5. Entity references

A targeted `ref` links one entity to another entity of a declared type. Here,
the task's `started` event records which `Worker` runs it.

## YAML model

```yaml
{{#include ../../examples/05-entity-references/model.yaml}}
```

## Instrumentation API

An entity handle produces a typed reference with `as_entity_ref`. The generated
`started` method only accepts a reference targeting `Worker`.

```rust
{{#include ../../examples/05-entity-references/src/main.rs}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Key point</strong>
    <p>An entity reference identifies a specific entity and preserves its type.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="05">
  <h2>Check yourself</h2>
  <fieldset data-answer="c" data-explanation="The ref target is part of the generated Rust type, so started requires a Worker reference.">
    <legend>Which entity type may the <code>worker</code> attribute target?</legend>
    <label><input type="radio" name="q05a" value="a"> Any entity</label>
    <label><input type="radio" name="q05a" value="b"> Only <code>Task</code></label>
    <label><input type="radio" name="q05a" value="c"> Only <code>Worker</code></label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="b" data-explanation="ref constrains the target type but does not add the tree-forming scope marker.">
    <legend>Does <code>ref: Worker</code> place <code>Task</code> under <code>Worker</code> in a hierarchy?</legend>
    <label><input type="radio" name="q05b" value="a"> Yes</label>
    <label><input type="radio" name="q05b" value="b"> No</label>
    <label><input type="radio" name="q05b" value="c"> Only when the event is <code>multi</code></label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
