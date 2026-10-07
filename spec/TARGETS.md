# Targets, build system, and optimization pipeline

## Build system

`tok build entry.tok -o app` loads the entry and its imports (relative files
and `pkg:` dependencies pinned by `tok.toml`/`tok.lock`), checks the whole
program, emits one Rust source file, and compiles it with `rustc` at
`opt-level=2` into a single self-contained executable. There is no separate
build file: the entry file and `tok.toml` are the build description.
`TOKIT_RUSTC` selects the Rust compiler and the active rustup toolchain
selects the host target. `tok run` interprets the same checked program
without compiling. Native runtime support is emitted only for the features a
program uses, which keeps binaries small
([native build pruning](../research/NATIVE_BUILD_PRUNING.md)).

## Optimization pipeline

```text
source -> parse -> check (types, effects, exhaustiveness)
       -> typed IR for eligible scalar functions (spec/TOKIT_IR.md)
       -> Rust emission (borrowed parameters, monomorphized generics,
          pruned runtime) -> rustc/LLVM -O2 -> executable
```

Tokit-level optimizations today are representation choices the checker
makes safe: immutable parameters are lent by reference, `Bytes` and closures
share storage until mutation, and pure scalar functions go through the typed
IR. Loop and inlining optimizations are delegated to LLVM through `rustc`.
Each optimization lands with a benchmark in `research/` and keeps
interpreter and native results identical.

## Target status

| Target | Status |
| --- | --- |
| Native executables (Linux, Windows, macOS hosts with Rust) | supported; tested on Linux CI and Windows |
| Reference interpreter (`tok run`) | supported everywhere the compiler runs |
| WebAssembly (`wasm32-wasip1`) | supported via `tok build file.tok -o app.wasm --target wasm32-wasip1`; tested in CI with wasmtime |
| GPU | not planned for this version |
| Embedded / `no_std` | not planned for this version |

### WebAssembly model

`tok build file.tok -o app.wasm --target wasm32-wasip1` passes the target to
`rustc` (install it with `rustup target add wasm32-wasip1`) and produces a
WASI module that runs under any WASI runtime, for example
`wasmtime run app.wasm args...`. Program arguments may be given without `--`.

Differences from native executables follow the concurrency model's allowed
schedules: there are no threads, so `main` runs on the module's own stack
(64 MiB, enough for the 10,000-call limit), `spawn` evaluates its pure
function at the spawn site, and `serve` with workers answers one request at a
time. Files need both a Tokit grant and a runtime preopen (`wasmtime run
--dir=. app.wasm --allow-read data.txt`). Network functions depend on the
runtime's WASI socket support and otherwise return `IoError::Other`.

### GPU and embedded

Tokit's value semantics and pure tasks are compatible with data-parallel
kernels, and its lack of a tracing GC suits small devices, but both need a
restricted subset (no strings, maps, or I/O in kernels; no heap or fixed heap
on microcontrollers) and a backend without `std`. They are deliberately out
of scope until the core language and its native target are stable.
