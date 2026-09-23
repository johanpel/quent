# Log

Sometimes you just want to simply log something human-readable. The log module
provides a simple way to declare an entity that behaves as a general purpose log
sink. At run-time, an instance of this entity will typically be leveraged as a
general-purpose logging API. It could also receive messages from an already
existing logging API that supports different backends.

The benefit is that those logs can follow the same export and analysis path as
all other types of events, such that they can be correlated at analysis-time.
For example, a UI could display them right alongside [resource utilization
metrics](../resource/index.md).

## YAML model

Add an entry under the top-level `logs:` key to declare a log sink. Each item
under `levels` creates a repeatable event with the same name. Quent adds a
`message: string` attribute to every level event. There is no separate level
attribute: an `info` message is an `info` event, and an `error` message is an
`error` event.

The order of `levels` sets their ranks. The first level has rank 0, the next has
rank 1, and so on.

```yaml
{{#include ../../../../../../crates/yaml/examples/log-sink/model.yaml}}
```

`AppLog` defines the levels and attributes supported by this kind of log sink.
Each `AppLog` instance has its own runtime identity.

Quent adds a required `message: string` attribute to every level event, so each
generated level method takes the message as an argument.

The `target`, `file`, `line`, `module`, and `thread_name` fields in this example
are arbitrary attributes. Because they are declared directly under
`logs.AppLog.attributes`, they are added to every level.

Attributes under a level are also arbitrary, but are added only to that level.
Here, `category` is added to `warning`, while `error_code` is added to `error`.

## Instrumentation API

The generated API has one method for each level. This example calls `info` and
`warning` directly. A backend for an existing logging API could instead
translate each log record into the matching generated method call.

```rust
{{#include ../../../../../../crates/yaml/examples/log-sink/src/main.rs:9:}}
```

```cpp
{{#include ../../../../../../experimental/vibe/codegen/cpp/example/tutorial/log-sink/main.cpp:6:}}
```

```python
{{#include ../../../../../../experimental/vibe/codegen/python/example/tutorial/log-sink/main.py:4:}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Key point</strong>
    <p>The entity identifies the sink. The event name identifies the level.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="log-sink">
  <h2>Check yourself</h2>
  <fieldset data-answer="b" data-explanation="Each declared level becomes a repeatable event with an implicit message attribute.">
    <legend>How does a log level appear in the generated schema?</legend>
    <label><input type="radio" name="qLogA" value="a"> As a runtime <code>log_level</code> attribute</label>
    <label><input type="radio" name="qLogA" value="b"> As a repeatable event named after the level</label>
    <label><input type="radio" name="qLogA" value="c"> As a separate entity</label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="a" data-explanation="Attributes under logs.&lt;name&gt;.attributes are added to every generated level event.">
    <legend>Where do you declare an arbitrary attribute used by every level?</legend>
    <label><input type="radio" name="qLogB" value="a"> Under <code>logs.&lt;name&gt;.attributes</code></label>
    <label><input type="radio" name="qLogB" value="b"> Under one item in <code>levels</code></label>
    <label><input type="radio" name="qLogB" value="c"> As a new log entity</label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>

## Full code

- [YAML model][yaml-model]
- [Rust source][rust-source]
- [C++ source][cpp-source]
- [Python source][python-source]

[yaml-model]: https://github.com/rapidsai/quent/blob/main/crates/yaml/examples/log-sink/model.yaml
[rust-source]: https://github.com/rapidsai/quent/blob/main/crates/yaml/examples/log-sink/src/main.rs
[cpp-source]: https://github.com/rapidsai/quent/blob/main/experimental/vibe/codegen/cpp/example/tutorial/log-sink/main.cpp
[python-source]: https://github.com/rapidsai/quent/blob/main/experimental/vibe/codegen/python/example/tutorial/log-sink/main.py
