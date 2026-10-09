# Task semantics: owner review for issue #42

This is a decision proposal, not a language guarantee. It records the current
prototype and the decisions needed before task syntax or runtime behavior is
frozen. See [issue #42](https://github.com/AlphaVIE/tokit/issues/42).

## What the prototype does today

- `spawn f(args)` accepts a named, non-generic function whose transitive calls
  are pure. Arguments are evaluated before spawning and copied into the task.
  Spawned closures and nested `spawn` are rejected (`E117`).
- Native tasks use OS threads with a 512 MiB reserved stack. The interpreter
  evaluates the function eagerly at `spawn`; it does not simulate scheduling.
- `Task<T>` is a copyable handle. The native runtime stores a result after the
  first `join`, and later joins, including joins through copies, return that
  same result. The interpreter also permits repeated joins. This is covered by
  `compiler/tests/tasks.rs`.
- `TaskError` has one variant, `Failed`. An uncaught host panic in a native
  task maps to it. Existing Tokit runtime diagnostics, such as overflow
  (`E201`), remain fatal. Native thread creation currently uses `expect`, so
  failure to start a thread does not return `TaskError::Failed`.
- No cancellation operation or language-level limit on live tasks exists.
  Dropping a native task's last handle drops its join handle; a running thread
  may continue until it completes or the process exits. The interpreter has
  already evaluated that task at the spawn site.

The [runtime model](../spec/RUNTIME_MODEL.md) describes the currently checked
subset. The [task-square baseline](baselines/task-square/RESULTS.md) verifies
only bounded successful results; it cannot establish failure behavior.

## Proposed decisions for the next experimental iteration

1. **Keep the current spawn form.** Use named functions and copied explicit
   arguments for now. Consider capture closures only after their capture,
   purity, and `Send`-like rules are designed together. This avoids making a
   syntax promise that the current two backends cannot validate equally.
2. **Keep repeatable joins.** Treat `Task<T>` as a shared handle to one cached
   result, rather than a single-use value. The implementation and tests
   already expose this behavior; changing it would require a migration plan.
3. **Keep fatal Tokit diagnostics fatal in all tasks.** Do not silently turn
   `E201` and similar diagnostics into `TaskError::Failed`, which would erase
   their code and source location. Reserve `Failed` for task infrastructure
   failures and uncaught host panics. Make native thread-start failure produce
   a completed failed task before promising this rule publicly.
4. **Do not add cancellation yet.** Define dropping an unjoined handle as
   discarding the result, without a guarantee that native work finishes before
   process exit. Because tasks are pure, successful completion has no
   language-visible side effect. Resource use and completion time remain
   observable to the host and should not be described as deterministic.
5. **Choose a resource bound before advertising unbounded parallelism.** The
   current one-thread-per-task approach can exhaust host threads or virtual
   address space. A bound or executor design should state what `spawn` and
   `join` return when capacity is exhausted. Keeping `spawn -> Task<T>` would
   allow a completed failed task, reported on `join`, without changing syntax.

## Evidence needed before closing #42

- Tests for repeated joins through copied handles on both backends, including
  a failed task where failure can be injected without relying on a Tokit
  runtime diagnostic.
- A controlled native thread-start failure test and matching interpreter
  contract for the resulting `TaskError::Failed`.
- Tests for fatal diagnostics inside tasks, dropped unjoined tasks, and a
  chosen resource limit. Keep successful results and failure modes separate.
- A decision on capture closures and on whether cancellation belongs in the
  first stable task API. Document any syntax or API migration before changing
  the existing prototype.

Owner review is required for the proposed failure, drop, and resource rules.
No runtime or checker behavior changes as a result of this document.
