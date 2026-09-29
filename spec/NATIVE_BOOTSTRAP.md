# Experimental native bootstrap

`tok build input.tok -o output` checks a Tokit program, emits Rust source from its AST and checked expression types, then calls `rustc` to produce a native executable. The generated source is created in a unique temporary file and removed after compilation. `TOKIT_RUSTC` can select the `rustc` executable. The selected rustup toolchain is inherited from the environment. This is a **bootstrap backend**, not a language specification or permanent Rust backend decision.

```text
Tokit source -> lexer/parser -> static checker -> typed expression map
             -> Rust emitter -> rustc -> host executable
```

The emitter generates code from AST nodes and hex-encodes user identifiers in Rust names. It does not paste Tokit source fragments into Rust. Supported behavior includes the current experimental primitive types, UTF-8 strings, records, unit enums, functions, recursion up to a conservative depth limit, local bindings, arrays and checked indexing, loops, branches, `Result`, `?`, and checked `i32` arithmetic. The executable prints its `main()` result using the same compact value rendering as the interpreter. The test suite compares native output with the interpreter on several examples and checks `E201` overflow, `E202` recursion, and `E205` bounds diagnostics.

The interpreter and generated binary currently reject calls beyond **32 active function frames** with `E202`. This low bound avoids stack exhaustion in the current recursive interpreter on Windows. It is a prototype limitation, not a final language guarantee; an iterative evaluator or a measured stack policy is needed.

## Reproduce

On a host with Rust 1.98.1 and a working linker:

```text
cargo run -p tokit-compiler --bin tok -- build examples/answer.tok -o answer
./answer
```

On the present Windows machine, select the installed GNU Rust toolchain and put Cargo's bin directory on `PATH`; the MSVC linker is not installed. The CI test job builds and executes native fixtures on Linux with `TOKIT_REQUIRE_NATIVE=1`.

## Limits and next backend work

This backend invokes an installed Rust compiler and inherits its target and optimization behavior. It does not yet define a stable ABI, a target-independent Tokit IR, a GC, native standard-library integration, reproducible binaries, or measured performance. Some well-typed programs may still reveal a Rust emission gap and report `E302`; such gaps need dedicated tests and fixes. A future backend interface should consume verified typed IR, permitting LLVM, Cranelift, WebAssembly, or another selected backend without altering source semantics. Backend selection requires measurements of compile latency, runtime, binary size, cross-platform behavior, and implementation cost.
