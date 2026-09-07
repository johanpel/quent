# 8. FSM self-loops

A transition from a state back to itself makes repeated entry valid. The FSM
topology therefore gives that state's generated event `multi` cardinality.

## YAML model

```yaml
{{#include ../../examples/08-fsm-self-loop/model.yaml}}
```

## Instrumentation API

The task enters `running` repeatedly as its progress changes, then enters
`completed` once.

```rust
{{#include ../../examples/08-fsm-self-loop/src/main.rs}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Key point</strong>
    <p>A self-loop makes the corresponding state-entry event repeatable.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="08">
  <h2>Check yourself</h2>
  <fieldset data-answer="a" data-explanation="The running-to-running transition means the state can be entered repeatedly, so its event is multi.">
    <legend>Why can <code>running</code> be emitted more than once?</legend>
    <label><input type="radio" name="q08a" value="a"> <code>running</code> has a self-loop</label>
    <label><input type="radio" name="q08a" value="b"> It has an attribute</label>
    <label><input type="radio" name="q08a" value="c"> It is the initial state</label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="b" data-explanation="No cycle can re-enter completed, so its derived event cardinality is once.">
    <legend>What cardinality is derived for <code>completed</code>?</legend>
    <label><input type="radio" name="q08b" value="a"> <code>multi</code></label>
    <label><input type="radio" name="q08b" value="b"> <code>once</code></label>
    <label><input type="radio" name="q08b" value="c"> No event is generated</label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
