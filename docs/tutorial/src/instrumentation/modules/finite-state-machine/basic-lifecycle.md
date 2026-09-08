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
Each state event takes a per-entity `u16` sequence number as its first argument.
This counter normally begins at zero and increments with each transition. It
orders transitions that receive the same timestamp and may wrap after its
maximum value. Passing this counter explicitly is temporary. Automatic
sequencing is work in progress in
[#416](https://github.com/rapidsai/quent/issues/416).

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

<section class="check-yourself" data-lesson="08">
  <h2>Check yourself</h2>
  <fieldset data-answer="b" data-explanation="The FSM validator checks the initial state, reachability, transition targets, and a final path.">
    <legend>Which property does the parser validate for this FSM?</legend>
    <label><input type="radio" name="q08a" value="a"> Exporter throughput</label>
    <label><input type="radio" name="q08a" value="b"> State reachability and a final path</label>
    <label><input type="radio" name="q08a" value="c"> Runtime thread ownership</label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="b" data-explanation="Both loading_input and restoring_checkpoint declare running in their to lists.">
    <legend>Which states can directly precede <code>running</code>?</legend>
    <label><input type="radio" name="q08b" value="a"> Only <code>queued</code></label>
    <label><input type="radio" name="q08b" value="b"> <code>loading_input</code> or <code>restoring_checkpoint</code></label>
    <label><input type="radio" name="q08b" value="c"> Only <code>loading_input</code></label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
