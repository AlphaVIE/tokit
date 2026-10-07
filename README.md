# Tokit

Tokit (Token + Kit) is a statically typed programming language designed for
the total cost of writing, checking, and repairing programs with language
models: few tokens, one canonical form, precise diagnostics, and native
executables.

```tokit
struct Item{name:String,price:F}
total(items:[Item])->F{fold(items,0.0,|sum,i|sum+i.price)}
main()->String{let cart=[Item("tea",3.5),Item("cake",4.25)];"total "+String(total(cart))}
```

```text
$ tok run cart.tok
"total 7.75"
$ tok build cart.tok -o cart && ./cart
"total 7.75"
```

## Quick start

With Rust 1.98 installed:

```text
cargo build --release -p tokit-compiler       # builds target/release/tok
tok new app && cd app && tok run main.tok      # a new project
tok add json                                   # a registry package
tok check --json main.tok                      # machine-readable diagnostics
```

Editors: the [VS Code extension](editors/vscode/README.md) and any LSP client
via `tok lsp`.

## The language

- Types: `i32`/`I`, `i64`/`L`, `f64`/`F`, `bool`, `String`, `Bytes`, arrays,
  ordered maps, `Option`, `Result`, records, enums, generics, and function
  values ([type system](spec/TYPE_SYSTEM.md)).
- Control flow: expression-oriented `if`, exhaustive `match` with nested
  patterns, `for`, `while`, `?` for errors, lambdas with `map`, `filter`,
  `fold`, `sort_by`.
- Values, not references: no null, no `unsafe`, no data races, no tracing GC;
  overflow, bounds, and other runtime failures stop with a code
  ([runtime model](spec/RUNTIME_MODEL.md)).
- Capabilities: files and network need explicit `--allow-read`,
  `--allow-write`, and `--allow-net` grants; `tok explain` lists a program's
  effects ([security model](spec/SECURITY_MODEL.md)).
- Library: text, maps, math, bit operations, hashing and Base64, standard
  I/O, environment and clocks, files, HTTP server and client, raw TCP,
  listeners, pure parallel tasks, and a worker-pool HTTP server.
- Packages: `json`, `http`, `redis`, `postgres`, `websocket` from the built-in
  [registry](spec/PACKAGE_REGISTRY.md), pinned by content hash.

The whole language fits in the [LLM guide](spec/LLM_GUIDE.md) (about 2,500
tokens), whose examples are checked by the test suite. All specifications are
indexed in [spec/README.md](spec/README.md); the grammar is in
[spec/GRAMMAR.md](spec/GRAMMAR.md) and every diagnostic in
[spec/DIAGNOSTICS.md](spec/DIAGNOSTICS.md).

## Tools

| Command | Purpose |
| --- | --- |
| `tok run`, `tok build`, `tok test`, `tok bench`, `tok repl` | run, compile natively or to WebAssembly (`--target wasm32-wasip1`), test, benchmark, explore |
| `tok check`, `tok lint`, `tok explain [--pseudo]`, `tok doc` | diagnostics, warnings, effects, human-readable views |
| `tok fmt`, `tok compact`, `tok expand` | canonical form and readable layout |
| `tok new`, `tok add`, `tok rm`, `tok search`, `tok lock` | projects and packages |
| `tok lsp` | language server: diagnostics, definitions, hover, references, rename, completion |

`tok --help` lists every option ([tools](spec/TOOLS.md)).

## Examples

[CLI calculator](examples/calculator.tok), [file processing](examples/csv_summary.tok),
[word count](examples/word_count.tok), [binary copy](examples/binary_copy.tok),
[JSON](examples/package_json/main.tok), [HTTP API](examples/http_server.tok),
[in-memory CRUD service](examples/crud_service.tok),
[concurrent HTTP service](examples/concurrent_http.tok),
[WebSocket echo](examples/websocket_echo/main.tok), [parallel tasks](examples/task_square.tok),
[closures](examples/closures.tok), and more in [examples/](examples/).

## Models

[research/finetune](research/finetune/README.md) holds a compiler-verified
dataset (train/validation/test splits), a LoRA training script, and an
evaluation harness that judges any model with `tok` itself, plus a
recommendation for getting good Tokit from Claude, GPT/Codex, or a local
model.

## Status

The implementation covers all sixteen phases of the [project brief](PROMPT.txt)
at least partially; see [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)
for evidence and remaining gaps (FFI, TLS, a remote registry, cross-file
editor navigation, and a self-hosted parser). Nothing is frozen as 1.0 yet. Design
background: [VISION](VISION.md), [design goals](DESIGN_GOALS.md),
[research plan](RESEARCH_PLAN.md), [measurements](research/).
