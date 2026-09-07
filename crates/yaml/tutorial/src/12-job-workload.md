# 12. Job workload

This capstone combines an FSM, event attributes, and measured resource usage.
A worker publishes its thread limit. A job records how many threads it requests
and how many it occupies while running.

## YAML model

```yaml
{{#include ../../examples/12-job-workload/model.yaml}}
```

## Instrumentation API

The generated API distinguishes the worker's `WorkerBounds` from the job's
`WorkerUsage`. No event names or payload keys are assembled at runtime.

```rust
{{#include ../../examples/12-job-workload/src/main.rs}}
```

<div class="badger-note">
  <div class="badger-mark" role="img" aria-label="Quent honey badger">
{{#include ../../../../ui/public/logo.svg}}
  </div>
  <div>
    <strong>Badger says</strong>
    <p>The job requests four threads and claims four threads. Accounting becomes less exciting when the numbers agree.</p>
  </div>
</div>

<section class="check-yourself" data-lesson="12">
  <h2>Check yourself</h2>
  <fieldset data-answer="b" data-explanation="WorkerBounds publishes the worker's available thread capacity.">
    <legend>What does <code>WorkerBounds { threads: 16 }</code> represent?</legend>
    <label><input type="radio" name="q12a" value="a"> Threads requested by one job</label>
    <label><input type="radio" name="q12a" value="b"> Threads available on the worker</label>
    <label><input type="radio" name="q12a" value="c"> Jobs completed by the worker</label>
    <p class="question-feedback"></p>
  </fieldset>
  <fieldset data-answer="c" data-explanation="running accepts the worker reference carrying the job's WorkerUsage claim.">
    <legend>Which call records where the job runs and how many threads it occupies?</legend>
    <label><input type="radio" name="q12b" value="a"> <code>worker.ready(...)</code></label>
    <label><input type="radio" name="q12b" value="b"> <code>job.queued(...)</code></label>
    <label><input type="radio" name="q12b" value="c"> <code>job.running(...)</code></label>
    <p class="question-feedback"></p>
  </fieldset>
  <button type="button" class="check-answers">Check answers</button>
  <p class="quiz-result" aria-live="polite"></p>
</section>
