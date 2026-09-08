# Event data

An event's `attributes` describe the data captured when that event occurs. The
generated event method receives one typed argument for each attribute, in
declaration order.

## Data types

The scalar types are:

| YAML type | Value |
| --- | --- |
| `bool` | `true` or `false` |
| `u8`, `u16`, `u32`, `u64` | Unsigned integers of the indicated width |
| `i8`, `i16`, `i32`, `i64` | Signed integers of the indicated width |
| `f32`, `f64` | Floating-point numbers of the indicated width |
| `string` | Text |
| `uuid` | A universally unique identifier |

Types can also be composed or refer to generated types:

| YAML type | Value |
| --- | --- |
| `{ option: T }` | A value of type `T` that may be absent |
| `{ list: T }` | An ordered collection of values of type `T` |
| A record name | An instance of that [record](records.md) |
| `dynamic` | String-keyed values whose names and types are chosen at runtime |
| `ref` | A reference to any entity instance |

Semantic modules add more specific reference forms. These are introduced with
[targeted references](../modules/reference-target/index.md),
[scoped references](../modules/reference-tree/index.md), and
[resources](../modules/resource/index.md).

## YAML model

```yaml
{{#include ../../../../../crates/yaml/examples/event-data/model.yaml}}
```

## Instrumentation API

The generated API maps each YAML type to the corresponding type in the selected
programming language. Options, lists, records, and references remain typed.
`dynamic` is the exception: it deliberately accepts values whose names and
types are determined at runtime.

```rust
{{#include ../../../../../crates/yaml/examples/event-data/src/main.rs}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Key point</strong>
    <p>Event attributes produce typed parameters in the generated instrumentation API.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="02">
  <h2>Check yourself</h2>
  <fieldset data-answer="a" data-explanation="An option explicitly represents a value that may be absent.">
    <legend>Which declaration permits an attribute value to be absent?</legend>
    <label><input type="radio" name="q02a" value="a"> <code>{ option: T }</code></label>
    <label><input type="radio" name="q02a" value="b"> <code>{ list: T }</code></label>
    <label><input type="radio" name="q02a" value="c"> <code>uuid</code></label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="b" data-explanation="A list contains an ordered collection of values with one element type.">
    <legend>Which declaration represents several ordered values of the same type?</legend>
    <label><input type="radio" name="q02b" value="a"> <code>{ option: T }</code></label>
    <label><input type="radio" name="q02b" value="b"> <code>{ list: T }</code></label>
    <label><input type="radio" name="q02b" value="c"> <code>dynamic</code></label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
