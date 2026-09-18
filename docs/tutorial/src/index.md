# Quent Tutorial

Quent uses an application event model to connect instrumentation with
domain-specific performance analysis. Its architecture follows four stages:

<div class="architecture-overview" role="img" aria-label="Quent architecture from an Application Event Schema through generated instrumentation and analysis libraries">
{{#include ../../figures/overview.svg}}
</div>

1. **Model the application.** An Application Event Schema defines the entities,
   events, and attributes that describe the application's behavior. Semantic
   modules add reusable constraints and meaning for analysis and user
   interfaces.
2. **Generate typed libraries.** Code generation produces an
   application-specific instrumentation library. A statically typed analysis
   library generated from the same schema is a work in progress.
3. **Capture events.** The application emits events through the generated
   instrumentation API. The instrumentation library writes those events to the
   event store. Other event sources can contribute to the same data.
4. **Analyze behavior.** Analysis services use schema semantics to interpret
   the stored events and provide application-specific results.

This tutorial builds an application event model from basic events through
finite-state machines and resources. Every lesson shows the complete YAML model
beside the generated instrumentation API that uses it.

Instrumentation examples are available in Rust, C++, and Python. Use the tabs
above each example to select a language; the selection carries across lessons.
The C++ and Python generators are currently experimental.

To keep the lessons focused, the displayed snippets omit license headers and
language-specific build wiring. The complete buildable sources remain available
on GitHub in the [Rust examples], [C++ examples], and [Python examples].

Use the [Quent Schema Explorer](https://rapidsai.github.io/quent/schema/) to
inspect an Application Event Schema interactively.

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Tutorial scope</strong>
    <p>Each example introduces one concept and shows the corresponding generated API.</p>
  </div>
</div>

Use the arrow on the right or the sidebar to begin.

[Rust examples]: https://github.com/rapidsai/quent/tree/main/crates/yaml/examples
[C++ examples]: https://github.com/rapidsai/quent/tree/main/experimental/vibe/codegen/cpp/example/tutorial
[Python examples]: https://github.com/rapidsai/quent/tree/main/experimental/vibe/codegen/python/example/tutorial
