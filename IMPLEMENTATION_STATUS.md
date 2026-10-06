# Implementation status

Status on 2026-10-06. The project brief defines phases 0–15.
**Eleven of sixteen phases have a runnable partial implementation**
(5–15). Five earlier phases have research or provisional specification
artifacts (0–4). This is a count of covered phases, not a percentage
of the language completed. None of the experimental contracts is a frozen
1.0 guarantee. Future progress updates should use the same sixteen-phase
denominator and revise this inventory when evidence changes.

| Phase | Current evidence | Status |
| ---: | --- | --- |
| 0 Problem definition | [Design goals](DESIGN_GOALS.md) | Research artifact |
| 1 Tokenizer research | [Initial measurements](research/INITIAL_RESULTS.md) | Research artifact; tokenizer coverage incomplete |
| 2 Syntax experiments | [Candidate comparison](LANGUAGE_CANDIDATES.md) | Research artifact; no selected public syntax |
| 3 Candidate benchmarks | [Research plan](RESEARCH_PLAN.md), [versioned benchmarks](research/GENERATED_ARRAY_CHASE.md) | Research artifact; generation and repair evaluation incomplete |
| 4 Language specification | [Experimental subset](spec/EXPERIMENTAL_SUBSET.md) | Provisional subset, not frozen |
| 5 Parser | [Lexer](compiler/src/lexer.rs), [parser](compiler/src/parser.rs), [tests](compiler/tests/front_end.rs) | Runnable partial implementation |
| 6 Semantic model | [Checker](compiler/src/checker.rs), [tests](compiler/tests/front_end.rs) | Runnable partial implementation |
| 7 Interpreter | [Reference interpreter](compiler/src/interpreter.rs), [tests](compiler/tests/i64.rs) | Runnable partial implementation |
| 8 Tokit IR | [Typed scalar IR](compiler/src/ir.rs), [contract](spec/TOKIT_IR.md), [tests](compiler/tests/ir.rs) | Runnable partial implementation; scalar expressions, immutable bindings, lazy conditional regions, and checked non-generic calls |
| 9 Native backend | [Rust bootstrap backend](compiler/src/native.rs), [contract](spec/NATIVE_BOOTSTRAP.md) | Runnable partial implementation; no independent optimizer/backend |
| 10 Runtime | [Native runtime helpers](compiler/src/native_runtime/core.rs.txt), [bytes](compiler/src/native_runtime/bytes.rs.txt) | Runnable partial implementation; GC and memory model remain open |
| 11 Standard library | [Experimental JSON module](examples/json/json.tok), [filesystem capability](spec/FILESYSTEM_CAPABILITY.md), [network capability](spec/NETWORK_CAPABILITY.md), [builtins](spec/EXPERIMENTAL_SUBSET.md) | Runnable partial implementation; strings, maps, arrays, math, standard I/O, environment, clocks, directories, and an HTTP/1.1 server and client; TLS and streaming absent |
| 12 Toolchain | [CLI](compiler/src/main.rs), [formatter](compiler/src/format.rs), [local packages](spec/LOCAL_PACKAGE_CANDIDATE.md) | Runnable partial implementation; registry/build ecosystem incomplete |
| 13 AI interfaces | [Program index](compiler/src/ai_index.rs), [function patch protocol](spec/AI_PATCH_BOOTSTRAP.md), [structured diagnostics](spec/AI_INDEX.md) | Runnable partial implementation; expression-level and multi-file AST edits absent |
| 14 IDE | [Experimental LSP server](spec/LSP_BOOTSTRAP.md), [implementation](compiler/src/lsp.rs) | Runnable partial implementation; diagnostics, symbols, definitions, hover, references, rename, and completion; cross-file navigation and bundled editor extension absent |
| 15 Self-hosting | [Self-hosted lexer](selfhost/lexer.tok), [contract](spec/SELF_HOSTING.md), [equivalence test](compiler/tests/selfhost_lexer.rs) | Runnable partial implementation; lexer only, parser and checker remain in Rust |

The current implementation supports a checked experimental subset, including
`i32`, `i64`, and `f64`, strings with a text library, ordered maps, arrays with
element assignment and higher-order builtins, lambdas, generic records and
enums, pattern matching on enums, integers, and strings, modules, local
packages, files, standard input and output, JSON, tasks, and native
executables. It does not yet satisfy the brief's broad
general-purpose, interoperability, IDE, and self-hosting goals. Open design
work is tracked in [task semantics](https://github.com/AlphaVIE/tokit/issues/42),
[JSON](https://github.com/AlphaVIE/tokit/issues/83), and
[packages](https://github.com/AlphaVIE/tokit/issues/88).
