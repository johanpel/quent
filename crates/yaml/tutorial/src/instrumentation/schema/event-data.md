# Event data

An event's `attributes` describe the data captured when that event occurs. The
generated event method receives one typed argument for each attribute, in
declaration order.

## YAML model

```yaml
{{#include ../../../../examples/event-data/model.yaml}}
```

## Instrumentation API

YAML scalar types map to ordinary Rust types. A YAML `string` becomes a Rust
`String`, while integer and boolean types retain their names.

```rust
{{#include ../../../../examples/event-data/src/main.rs}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Key point</strong>
    <p>Event attributes produce typed parameters in the generated instrumentation API.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="02">
  <h2>Check yourself</h2>
  <fieldset data-answer="a" data-explanation="Event attributes become typed arguments on the generated event method.">
    <legend>What does the <code>command</code> attribute become in the generated API?</legend>
    <label><input type="radio" name="q02a" value="a"> An argument to <code>started</code></label>
    <label><input type="radio" name="q02a" value="b"> A global variable</label>
    <label><input type="radio" name="q02a" value="c"> An exporter setting</label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="b" data-explanation="The YAML string type maps to Rust's owned String type.">
    <legend>Which Rust type is generated for YAML <code>string</code>?</legend>
    <label><input type="radio" name="q02b" value="a"> <code>&amp;str</code></label>
    <label><input type="radio" name="q02b" value="b"> <code>String</code></label>
    <label><input type="radio" name="q02b" value="c"> <code>Vec&lt;u8&gt;</code></label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
