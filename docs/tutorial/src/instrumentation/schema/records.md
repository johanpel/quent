# Records

A record groups related fields into a named, reusable type. Event attributes
can use a record name wherever they can use a scalar type.

## YAML model

```yaml
{{#include ../../../../../crates/yaml/examples/records/model.yaml}}
```

## Instrumentation API

The generated API represents `TaskResult` as a Rust struct. Construct the
record, then pass it to the event method.

```rust
{{#include ../../../../../crates/yaml/examples/records/src/main.rs}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Key point</strong>
    <p>A named record provides one reusable structure for event attributes.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="04">
  <h2>Check yourself</h2>
  <fieldset data-answer="b" data-explanation="A YAML record is generated as a Rust struct with public typed fields.">
    <legend>What Rust item is generated for <code>TaskResult</code>?</legend>
    <label><input type="radio" name="q04a" value="a"> An enum variant</label>
    <label><input type="radio" name="q04a" value="b"> A struct</label>
    <label><input type="radio" name="q04a" value="c"> A trait</label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="a" data-explanation="Records give a group of related fields one reusable, generated type.">
    <legend>Why declare a record instead of repeating its fields?</legend>
    <label><input type="radio" name="q04b" value="a"> To reuse one named field group</label>
    <label><input type="radio" name="q04b" value="b"> To make the event repeatable</label>
    <label><input type="radio" name="q04b" value="c"> To choose an exporter</label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
