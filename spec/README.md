# Tokit specifications

The project brief requires 32 specification topics before the architecture
is considered stable. This index maps each one to the document that covers
it and says whether the behavior is implemented. "Implemented" means tested
in both backends; nothing here is a frozen 1.0 guarantee yet.

| # | Topic | Document | State |
| ---: | --- | --- | --- |
| 1 | Language philosophy | [DESIGN_GOALS](../DESIGN_GOALS.md), [VISION](../VISION.md) | specified |
| 2 | Lexical grammar | [GRAMMAR](GRAMMAR.md) | implemented |
| 3 | Formal grammar (EBNF) | [GRAMMAR](GRAMMAR.md) | implemented |
| 4 | Type system | [TYPE_SYSTEM](TYPE_SYSTEM.md) | implemented |
| 5 | Numeric semantics | [EXPERIMENTAL_SUBSET](EXPERIMENTAL_SUBSET.md) (`i32`, `i64`, `f64`, bit operations) | implemented |
| 6 | String semantics | [EXPERIMENTAL_SUBSET](EXPERIMENTAL_SUBSET.md) (strings, `Bytes`, UTF-8) | implemented |
| 7 | Memory model | [RUNTIME_MODEL](RUNTIME_MODEL.md) | implemented |
| 8 | GC model | [RUNTIME_MODEL](RUNTIME_MODEL.md) — ownership instead of tracing GC | implemented |
| 9 | Concurrency model | [RUNTIME_MODEL](RUNTIME_MODEL.md) | implemented |
| 10 | Error model | [RUNTIME_MODEL](RUNTIME_MODEL.md) | implemented |
| 11 | Module system | [MODULE_SYSTEM_CANDIDATE](MODULE_SYSTEM_CANDIDATE.md) | implemented |
| 12 | Generics | [TYPE_SYSTEM](TYPE_SYSTEM.md) | implemented |
| 13 | Traits / interfaces | [TYPE_SYSTEM](TYPE_SYSTEM.md) — none, by decision | decided |
| 14 | Unsafe model | [RUNTIME_MODEL](RUNTIME_MODEL.md) — no `unsafe` | decided |
| 15 | Capability model | [SECURITY_MODEL](SECURITY_MODEL.md), [filesystem](FILESYSTEM_CAPABILITY.md), [network](NETWORK_CAPABILITY.md) | implemented |
| 16 | FFI | [RUNTIME_MODEL](RUNTIME_MODEL.md) | not yet |
| 17 | Compiler architecture | [COMPILER_ARCHITECTURE](../COMPILER_ARCHITECTURE.md), [NATIVE_BOOTSTRAP](NATIVE_BOOTSTRAP.md) | implemented |
| 18 | Tokit IR | [TOKIT_IR](TOKIT_IR.md) | partial (scalar functions) |
| 19 | Optimization pipeline | [TARGETS](TARGETS.md) | implemented |
| 20 | Standard library | [EXPERIMENTAL_SUBSET](EXPERIMENTAL_SUBSET.md), [LLM_GUIDE](LLM_GUIDE.md) (builtin list) | implemented |
| 21 | Package manager | [PACKAGE_REGISTRY](PACKAGE_REGISTRY.md), [LOCAL_PACKAGE_CANDIDATE](LOCAL_PACKAGE_CANDIDATE.md) | implemented (embedded registry) |
| 22 | Build system | [TARGETS](TARGETS.md) | implemented |
| 23 | AI protocol | [AI_INDEX](AI_INDEX.md), [LLM_GUIDE](LLM_GUIDE.md) | implemented |
| 24 | AST patch protocol | [AI_PATCH_BOOTSTRAP](AI_PATCH_BOOTSTRAP.md) | implemented (function level) |
| 25 | Compiler diagnostics | [DIAGNOSTICS](DIAGNOSTICS.md), [SOURCE_LOCATIONS](SOURCE_LOCATIONS.md) | implemented |
| 26 | Canonical formatting | [FORMATTER](FORMATTER.md) | implemented |
| 27 | Security model | [SECURITY_MODEL](SECURITY_MODEL.md) | implemented |
| 28 | WebAssembly model | [TARGETS](TARGETS.md) | implemented (`wasm32-wasip1`) |
| 29 | GPU model | [TARGETS](TARGETS.md) | out of scope for now |
| 30 | Embedded model | [TARGETS](TARGETS.md) | out of scope for now |
| 31 | Testing model | [TEST_RUNNER](TEST_RUNNER.md) | implemented |
| 32 | Benchmark methodology | [TOKEN_BENCHMARK_SPEC](../TOKEN_BENCHMARK_SPEC.md), [RESEARCH_PLAN](../RESEARCH_PLAN.md) | implemented |

Tooling: [TOOLS](TOOLS.md) (`tok doc/lint/bench/repl/expand`),
[LSP_BOOTSTRAP](LSP_BOOTSTRAP.md) (editor support), [SELF_HOSTING](SELF_HOSTING.md).
