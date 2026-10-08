# Tokit

**A compact, statically typed language built for writing code with language
models.** Few tokens, one canonical form, diagnostics a model can act on, and
native executables as fast as Rust.

[Language guide](spec/LLM_GUIDE.md) ·
[Specifications](spec/README.md) · [Examples](examples/) ·
[Releases](https://github.com/AlphaVIE/tokit/releases)

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

## Why Tokit

- **Fewer tokens.** On the five executable [baselines](research/baselines),
  the core functions take 182 tokens in Tokit against 226 in Rust, 246 in
  Python, 273 in Go, and 327 in TypeScript (o200k_base; reproduce with
  `scripts/compare_baseline_tokens.py`). Small tasks, so read it as a
  direction, not a law.
- **Errors a model can fix in one step.** Every diagnostic has a stable code,
  a position, and a concrete message — `non-exhaustive match: missing
  Some(false)`, `call len(x)` for `x.len()` — available as JSON from
  `tok check --json` ([all codes](spec/DIAGNOSTICS.md)).
- **One way to write it.** `tok fmt` and `tok compact` produce a single
  canonical form, so generated code diffs cleanly and needs no style review.
- **Safe by construction.** Values instead of references: no null, no
  `unsafe`, no data races, no garbage collector pauses. Overflow and bounds
  errors stop with a code instead of corrupting memory.
- **Explicit capabilities.** Files, network, and processes need
  `--allow-read`, `--allow-write`, `--allow-net`, or `--allow-run`;
  `tok explain` lists everything a program can do before you run it.
- **Fast.** `tok build` emits Rust and compiles it with LLVM; on the measured
  loops Tokit and Rust run within 1% of each other. WebAssembly via
  `--target wasm32-wasip1`.

## Install

```text
curl -sSf https://raw.githubusercontent.com/AlphaVIE/tokit/main/scripts/install.sh | sh     # Linux, macOS
irm https://raw.githubusercontent.com/AlphaVIE/tokit/main/scripts/install.ps1 | iex          # Windows
```

The installers download a prebuilt `tok` from the latest
[release](https://github.com/AlphaVIE/tokit/releases), verify its SHA-256, and
install it into `~/.tok/bin`. Everything works right away except native
compilation: `tok build` and `tok run --native` also need
[Rust](https://rustup.rs). The VS Code extension (`tokit-vscode.vsix`) is
attached to each release; other editors can run `tok lsp`.

To build from source: `cargo build --release -p tokit-compiler` (Rust 1.98).

## Quick start

```text
tok new hello && cd hello
tok run main.tok
tok add json                      # a package from the built-in registry
tok check --json main.tok         # machine-readable diagnostics
tok build main.tok -o hello       # a native executable
```

## The language at a glance

```tokit
enum Shape{Circle(F),Rect(Size)}
struct Size{w:F,h:F}
area(s:Shape)->F{match s{Shape::Circle(r)=>pi()*r*r,Shape::Rect(z)=>z.w*z.h}}
parse(text:String)->Result<I,ParseError>{Ok(parse_i32(trim(text))?*2)}
main()->[String]{let shapes=[Shape::Circle(1.0),Shape::Rect(Size(2.0,3.0))];map(shapes,|s|String(area(s)))+[match parse(" 21 "){Ok(n)=>String(n),Err(ParseError::Invalid)=>"invalid",Err(e)=>"range"}]}
```

- Types: `I`/`i32`, `L`/`i64`, `F`/`f64`, `bool`, `String`, `Bytes`, arrays,
  ordered `Map`, `Option`, `Result`, records, enums, generics, and function
  values ([type system](spec/TYPE_SYSTEM.md), [grammar](spec/GRAMMAR.md)).
- Expressions everywhere: `if` and `match` return values; patterns nest;
  `?` propagates errors; `==` compares structurally.
- Library: text, maps, math, bit operations, hashing and Base64, files,
  standard I/O, HTTP and HTTPS clients, HTTP servers with worker pools, TCP,
  WebSocket, subprocesses, and pure parallel tasks.
- Packages: `json`, `http`, `redis`, `postgres`, and `websocket` ship with the
  compiler; any Git repository can be a package
  (`tok add name --git <url>`), pinned by commit and content hash
  ([packages](spec/PACKAGE_REGISTRY.md)).

The complete language fits in the [LLM guide](spec/LLM_GUIDE.md), about 2,500
tokens, and every example in it is compiled and run by the test suite.

## Tools

| Command | Purpose |
| --- | --- |
| `tok run`, `tok run --native` | interpret, or compile once and run from a cache |
| `tok build [--target wasm32-wasip1]` | native executable or WebAssembly module |
| `tok test`, `tok bench`, `tok repl` | tests, benchmarks, interactive exploration |
| `tok check`, `tok lint`, `tok explain`, `tok doc` | diagnostics, warnings, effects, API docs |
| `tok fmt`, `tok compact`, `tok expand` | canonical form, and a readable layout for review |
| `tok new`, `tok add`, `tok rm`, `tok search`, `tok lock` | projects and packages |
| `tok lsp` | diagnostics, completion, definitions, hover, references, rename |

`tok --help` lists every option ([tools](spec/TOOLS.md)).

## Examples

[Calculator](examples/calculator.tok) ·
[CSV processing](examples/csv_summary.tok) ·
[word count](examples/word_count.tok) ·
[binary files](examples/binary_copy.tok) ·
[JSON](examples/package_json/main.tok) ·
[HTTP API](examples/http_server.tok) ·
[CRUD service](examples/crud_service.tok) ·
[concurrent HTTP](examples/concurrent_http.tok) ·
[WebSocket echo](examples/websocket_echo/main.tok) ·
[parallel tasks](examples/task_square.tok)

## Writing Tokit with AI

Give any capable model `spec/LLM_GUIDE.md` as its system prompt and let it
call `tok check --json` until the program compiles. To train your own model,
[research/finetune](research/finetune/README.md) provides a compiler-verified
dataset with train, validation, and test splits, a QLoRA script for a single
16 GB GPU, Ollama packaging, and an evaluation harness that scores any model
with `tok` itself.

## Status

Version 0.1: usable for command-line tools, services, and data processing,
tested on Linux, macOS, Windows, and WebAssembly. The language is not frozen;
breaking changes are still possible before 1.0. Open work is tracked in
[IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md); design background is in
[VISION](VISION.md), [DESIGN_GOALS](DESIGN_GOALS.md), and [research](research/).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option. This covers the compiler, the runtime embedded in compiled
programs, the packages, the specifications, and the dataset. Programs you
write and the binaries `tok build` produces are yours. Contributions are
accepted under the same terms.
