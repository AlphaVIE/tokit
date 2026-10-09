# Runtime model

Memory, garbage collection, concurrency, errors, `unsafe`, and FFI — what the
two backends ([interpreter](../compiler/src/interpreter.rs),
[native](../compiler/src/native.rs)) guarantee today.

## Memory model: values, not references

Every Tokit value behaves as an independent value. Assigning, passing,
returning, capturing in a lambda, or storing into an array, record, or map
copies it observably: changing one binding never changes another. There are
no references, pointers, aliases, or address-of operations in the language,
and no way to observe identity except for the explicitly shared handles
below.

Implementations may share storage as long as no program can tell:

- `Bytes` and closures are reference counted and copied on first mutation.
- Native code passes non-scalar parameters a function never mutates by
  borrow instead of by copy ([borrowed parameters](../research/BORROWED_PARAMETERS.md)).
- Arrays, strings, records, and maps are otherwise copied when a second
  binding needs them; programs that rebuild large values in loops pay for it.

The only shared, mutable runtime objects are handles whose sharing is part of
their meaning: `Task` (one cached result, observable through repeated joins),
`Conn` and `Listener` (one socket). Copies of a handle refer to the same
object. The broader task failure and cancellation contract is still under
[review](../research/TASK_SEMANTICS_REVIEW.md).

## GC model: no tracing collector

Phase 10 of the brief asks for a GC. Tokit's value semantics make a tracing
collector unnecessary:

- A value can contain other values only by copy, and recursion in types must
  go through arrays (`E112`), so every value is a finite tree.
- Lambdas capture copies of the locals they read, never the enclosing
  environment, so a closure cannot reach itself.
- Handles hold operating-system resources, not Tokit values.

With no possible cycles, ownership (native: Rust ownership and `Arc`;
interpreter: Rust ownership and `Rc`) frees every value deterministically
when its last owner goes away. There are no GC pauses, no finalizers, and no
write barriers, and memory use is a function of live values only. The
trade-off is copying cost for large values that are shared and then changed;
the backends reduce it with the sharing rules above. A future cyclic data
structure (graphs with back references) would be expressed with indexes into
arrays or maps, which is also the representation the IR can optimize.

Sockets close when `tcp_close` is called or when the process exits; dropping
the last copy of a `Conn` does not close it yet.

## Concurrency model

- `spawn f(args)` runs a named, non-generic, **pure** function as a task and
  returns `Task<T>`; `join(t)` returns `Result<T,TaskError>`. Pure means the
  function and everything it calls perform no effect (I/O, clocks,
  randomness, network, `exit`); violations report `E117` at compile time.
  Arguments are copied in, so tasks share nothing.
- Native tasks run on OS threads with a 512 MiB reserved stack. The reference
  interpreter evaluates a task eagerly at the spawn site; because tasks are
  pure, both schedules produce the same result.
- `serve(addr,limit,workers,handler)` answers HTTP requests on a pool of
  native threads. Handlers may perform effects, but they receive copies of
  their captures, so the only nondeterminism is the interleaving of their
  effects (output order, timing). The interpreter answers one request at a
  time.
- There are no locks, atomics, channels, or shared mutable variables in the
  language, so data races are impossible by construction.

## Error model

- Expected failures are values: `Result<T,E>` with typed error enums
  (`IoError`, `ParseError`, `TaskError`, or user enums) and `Option<T>`.
  `?` propagates them; `match` handles them exhaustively.
- Failures that typing cannot rule out stop the program with a diagnostic
  and exit status 1: integer overflow and division by zero (`E201`), call
  depth over 10,000 (`E202`), index or slice out of range (`E205`), array
  length over `i32` (`E206`), byte value outside 0–255 (`E207`). There is no
  unwinding that user code can observe, catch, or resume.
- `exit(code)` flushes standard output and ends the process.

Both backends report the same code at the same source position for the same
program and input.

## Unsafe model

There is no `unsafe` construct. The language has no raw memory access,
unchecked indexing, unchecked arithmetic, or transmutation. Safety-relevant
operations that the runtime implements in Rust (sockets, files, threads,
hashing) are exposed only through checked builtins with documented failure
values.

## FFI

There is no in-process foreign function interface. Interoperability goes
through processes and protocols: `exec` runs any granted program and
captures its output ([process capability](PROCESS_CAPABILITY.md)), plus
standard input and output, files, HTTP, TCP, and libraries written in Tokit on top of them (JSON, Redis, PostgreSQL,
WebSocket in the [registry](PACKAGE_REGISTRY.md)). A future FFI would bind
Rust or C functions through declared signatures and an explicit capability
grant, so that foreign code stays visible in `tok explain` effect reports;
it is not designed yet.

## Determinism

For a given program, input, and grants, output is deterministic except where
the environment is read (`now_ms`, `clock_ns`, `random_bytes`, `env`, file
system, network) or where `serve` runs handlers concurrently. Map iteration
order is sorted by key, float formatting is fixed, and hash-based iteration
is never exposed.
