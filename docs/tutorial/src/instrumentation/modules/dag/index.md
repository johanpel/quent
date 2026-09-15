# Directed Acyclic Graph

The Directed Acyclic Graph semantic module marks entity types as DAGs,
vertices, or directed edges. A vertex identifies its containing DAG through an
`in` reference. An edge identifies its containing DAG and its source and target
vertex types.

## YAML model

```yaml
{{#include ../../../../../../crates/yaml/examples/dag/model.yaml}}
```

The `plan`, `source`, and `target` attributes are targeted entity references.
The parser requires each vertex and edge to declare its DAG membership in a
once-event. An edge must declare its membership, source, and target in the same
once-event. Both endpoint types must be vertices of the same DAG type.

These checks validate the schema topology. Whether emitted edges connect
vertices in the same DAG instance and whether those edges form an acyclic graph
are properties of the reconstructed event data.

## Instrumentation API

The generated methods accept typed entity references. The DAG roles add meaning
to the model without introducing separate DAG-specific methods.

```rust
{{#include ../../../../../../crates/yaml/examples/dag/src/main.rs}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Key point</strong>
    <p>DAG topology is recorded through typed entity references on ordinary events.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="dag">
  <h2>Check yourself</h2>
  <fieldset data-answer="b" data-explanation="The source declaration targets Operator, so the generated parameter accepts an Operator reference.">
    <legend>Which reference type does <code>PlanEdge.connected</code> accept for <code>source</code>?</legend>
    <label><input type="radio" name="qDagA" value="a"> <code>EntityRef&lt;Plan&gt;</code></label>
    <label><input type="radio" name="qDagA" value="b"> <code>EntityRef&lt;Operator&gt;</code></label>
    <label><input type="radio" name="qDagA" value="c"> Any entity reference</label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="c" data-explanation="Acyclicity depends on the emitted entity references and is checked after event data is reconstructed.">
    <legend>When can Quent determine whether the emitted plan contains a cycle?</legend>
    <label><input type="radio" name="qDagB" value="a"> While parsing the YAML alone</label>
    <label><input type="radio" name="qDagB" value="b"> While compiling the generated method call</label>
    <label><input type="radio" name="qDagB" value="c"> After reconstructing the emitted event data</label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
