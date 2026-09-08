# Quent Tutorial

This tutorial builds a Quent model from basic events through finite-state
machines and resources. Every lesson shows the complete YAML model beside the
generated instrumentation API that uses it.

## How Quent fits together

<div class="architecture-overview" role="img" aria-label="Quent architecture from an Application Event Schema through generated instrumentation and analysis libraries">
{{#include ../../figures/overview.svg}}
</div>

1. **Model the application.** An Application Event Schema defines the entities,
   events, and attributes that describe the application's behavior. Semantic
   modules add reusable constraints and meaning for analysis and user
   interfaces.
2. **Generate typed libraries.** Code generation produces application-specific
   instrumentation and analysis libraries from the same schema.
3. **Capture events.** The application emits events through the generated
   instrumentation API. The instrumentation library writes those events to the
   event store. Other event sources can contribute to the same data.
4. **Analyze behavior.** An analysis service uses the generated analysis
   library to interpret the stored events and provide application-specific
   results.

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
