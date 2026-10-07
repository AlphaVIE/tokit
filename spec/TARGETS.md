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
| WebAssembly | not yet; see below |
| GPU | not planned for this version |
| Embedded / `no_std` | not planned for this version |

### WebAssembly model

The emitted Rust is plain `std` Rust, so `wasm32-wasip1` is the natural first
WebAssembly target: a future `tok build --target wasm32-wasip1` would pass the
target to `rustc` and map capabilities to WASI preopens (files) and
host-provided sockets. Native tasks use OS threads and `serve` uses a thread
pool; on WebAssembly without threads they would run with the interpreter's
schedule (eager pure tasks, one request at a time), which the concurrency
model already allows. Network capabilities depend on host support for WASI
sockets. None of this is implemented or tested yet.

### GPU and embedded

Tokit's value semantics and pure tasks are compatible with data-parallel
kernels, and its lack of a tracing GC suits small devices, but both need a
restricted subset (no strings, maps, or I/O in kernels; no heap or fixed heap
on microcontrollers) and a backend without `std`. They are deliberately out
of scope until the core language and its native target are stable.
