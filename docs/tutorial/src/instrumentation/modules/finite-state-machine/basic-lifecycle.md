# Basic lifecycle

An FSM puts lifecycle topology in the model. It declares an initial state,
allowed transitions, and a reachable final state.

## YAML model

```yaml
{{#include ../../../../../../crates/yaml/examples/finite-state-machine/model.yaml}}
```

The parser rejects missing initial states, unreachable states, invalid targets,
and FSMs without a reachable final state. It also derives event cardinality from
the topology.

## Instrumentation API

Entering a state emits its generated event. The parser validates the transition
topology, while generated handles do not enforce transition order at runtime.

```rust
{{#include ../../../../../../crates/yaml/examples/finite-state-machine/src/main.rs}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Key point</strong>
    <p>The model declares valid transitions, reachable states, and the path to a final state.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="07">
  <h2>Check yourself</h2>
  <fieldset data-answer="b" data-explanation="The FSM validator checks the initial state, reachability, transition targets, and a final path.">
    <legend>Which property does the parser validate for this FSM?</legend>
    <label><input type="radio" name="q07a" value="a"> Exporter throughput</label>
    <label><input type="radio" name="q07a" value="b"> State reachability and a final path</label>
    <label><input type="radio" name="q07a" value="c"> Runtime thread ownership</label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="c" data-explanation="The generated API emits state-entry events but does not enforce transition order at runtime.">
    <legend>Does the Rust handle prevent calling <code>completed</code> before <code>running</code>?</legend>
    <label><input type="radio" name="q07b" value="a"> Yes, through typestate</label>
    <label><input type="radio" name="q07b" value="b"> Yes, by blocking the thread</label>
    <label><input type="radio" name="q07b" value="c"> No</label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
