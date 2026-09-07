# Minimal model

Every model declares the YAML format version and a model name. This model has
one entity type, `Task`, with two events. Events are emitted at most once per
entity instance unless the model says otherwise.

## YAML model

```yaml
{{#include ../../../../../crates/yaml/examples/minimal-model/model.yaml}}
```

## Instrumentation API

The context provides an observer for each entity type. A handle represents one
entity instance and exposes one method per event.

```rust
{{#include ../../../../../crates/yaml/examples/minimal-model/src/main.rs}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Key point</strong>
    <p>The two events have no declared ordering constraint, so either event may be emitted first.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="01">
  <h2>Check yourself</h2>
  <fieldset data-answer="b" data-explanation="Events are once by default. Use multi: true when repetition is part of the model.">
    <legend>How often can <code>started</code> be emitted for one <code>Task</code> handle?</legend>
    <label><input type="radio" name="q01a" value="a"> Any number of times</label>
    <label><input type="radio" name="q01a" value="b"> Once</label>
    <label><input type="radio" name="q01a" value="c"> Once for the entire process</label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="c" data-explanation="A handle identifies one entity instance and emits that instance's events.">
    <legend>What does <code>task</code> represent in the Rust program?</legend>
    <label><input type="radio" name="q01b" value="a"> The complete model</label>
    <label><input type="radio" name="q01b" value="b"> Every <code>Task</code> entity</label>
    <label><input type="radio" name="q01b" value="c"> One <code>Task</code> entity instance</label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
