# Preliminary compiler architecture

No grammar or backend is committed by this document. The initial compiler implementation language is Rust, following the project brief, once a subset is approved and Rust tooling is available.

```text
.tok source -> lexer -> parser -> lossless syntax tree -> resolved/typed model
                                                |                  |
                                            formatter          diagnostics
                                                                   |
                           explain/index <- semantic API <- typed IR verifier
                                                                   |
                                      optimization passes -> backend interface
                                                                   |
                                                          native/Wasm artifacts
```

## Boundaries

- **Syntax tree:** preserves spans and trivia so formatting and diagnostics can report source locations. Parsing must be deterministic and recover enough to show multiple errors. Stable semantic identities are a separate problem from source offsets.
- **Resolved and typed model:** owns names, types, explicit effects/capabilities if adopted, and typed errors. It rejects implicit incompatible arithmetic and unrestricted null. The exact type system remains open.
- **IR:** backend-neutral operations with explicit types and control flow. Include allocation and effects as explicit operations so later memory strategies need not be encoded into source grammar. Verify IR invariants before optimization.
- **Backends:** keep target-specific layout, ABI, object emission, and optimizers behind an interface. Evaluate LLVM against alternatives using implementation effort, output performance, compile latency, tooling, and distribution cost.
- **Runtime:** define a narrow ABI for allocation, scheduling, panic/fatal failure, and foreign calls. A practical tracing GC is the initial direction in the brief, but its precise design needs prototypes.
- **CLI:** one `tok` entry point can offer `check`, `fmt`, `stats`, `explain`, `build`, `run`, and `test` incrementally. Commands must report unsupported features honestly.

## Machine-oriented interfaces

Diagnostics should have a stable code, severity, span, and structured payload. JSON output is an interoperability format; compact AI output can be a separate rendering of the same diagnostic. The semantic index and human explanation must derive from the checked model, never from heuristics over raw text.

AST patching needs semantic node IDs, base revision checks, transactions, atomic validation, and rollback. Source maps connect edits to Git diffs. Do not expose the patch protocol as stable before edits across formatting and nearby insertion have been tested.

## Dependency and build boundary

The compiler reads explicit manifest and lock inputs. Dependency resolution, downloads, and signing verification stay outside semantic analysis; the compiler receives a closed dependency graph. Builds should record compiler version, target, flags, and content hashes. Reproducibility is tested by repeated artifact hashes after excluding explicitly documented nondeterministic metadata.

## First vertical slice

After grammar review: parse a file with explicit integer and boolean types, functions, local bindings, arithmetic, and conditional return; type-check it; run through a reference interpreter or native backend; emit both human and structured diagnostics. This slice is deliberately narrower than the MVP and establishes end-to-end correctness.
