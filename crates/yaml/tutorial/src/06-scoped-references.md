# 6. Scoped references

A `scope-ref` is a typed entity reference that also defines a parent
relationship. The parser validates all scoped references together as one tree.

## YAML model

```yaml
{{#include ../../examples/06-scoped-references/model.yaml}}
```

## Instrumentation API

The Rust value is obtained from the parent entity's handle. The additional
hierarchy meaning belongs to the model and its constraints.

```rust
{{#include ../../examples/06-scoped-references/src/main.rs}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Key point</strong>
    <p>A scoped reference defines a parent relationship in a validated entity tree.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="06">
  <h2>Check yourself</h2>
  <fieldset data-answer="a" data-explanation="scope-ref adds the tree-forming constraint in addition to the target constraint.">
    <legend>What does <code>scope-ref</code> add beyond a normal targeted <code>ref</code>?</legend>
    <label><input type="radio" name="q06a" value="a"> A validated parent relationship</label>
    <label><input type="radio" name="q06a" value="b"> Repeated event cardinality</label>
    <label><input type="radio" name="q06a" value="c"> Resource bounds</label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="c" data-explanation="The Pipeline handle produces the typed entity reference accepted by Task.started.">
    <legend>Which value is passed as the task's parent?</legend>
    <label><input type="radio" name="q06b" value="a"> The pipeline observer</label>
    <label><input type="radio" name="q06b" value="b"> The complete context</label>
    <label><input type="radio" name="q06b" value="c"> A reference from the pipeline handle</label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
