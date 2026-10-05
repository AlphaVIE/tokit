# Tokit

Tokit explores a machine-first programming language optimized for the total cost of producing, checking, and repairing programs with language models. The language design is experimental; no source syntax has been selected yet.

The project brief is in [PROMPT.txt](PROMPT.txt). Start with [VISION.md](VISION.md), [DESIGN_GOALS.md](DESIGN_GOALS.md), [RESEARCH_PLAN.md](RESEARCH_PLAN.md), and the [current implementation status](IMPLEMENTATION_STATUS.md). The [candidate comparison](LANGUAGE_CANDIDATES.md) and [initial measurements](research/INITIAL_RESULTS.md) are experiments, not a frozen syntax.

An [experimental candidate A compiler slice](spec/EXPERIMENTAL_SUBSET.md) can parse, type-check, interpret, explain, [format source](spec/FORMATTER.md), [measure structural density](research/STRUCTURAL_METRICS.md), and [build native executables](spec/NATIVE_BOOTSTRAP.md) from a small `.tok` subset. With Rust installed, run `cargo test --workspace`, `cargo run -p tokit-compiler --bin tok -- run examples/answer.tok`, or `cargo run -p tokit-compiler --bin tok -- explain examples/match_result.tok`. The source grammar and bootstrap backend are not yet production language commitments.

An [experimental language server](spec/LSP_BOOTSTRAP.md) runs as `tok lsp` and provides live diagnostics and document symbols for `.tok` buffers, including unsaved relative imports and pinned package sources.

An experimental [function patch protocol](spec/AI_PATCH_BOOTSTRAP.md) lets agents submit hash-guarded, checked changes to named functions without resending their surrounding source file.

The native bootstrap uses an initial [typed scalar IR](spec/TOKIT_IR.md) for pure `i32`, `i64`, and `bool` expression functions. Other checked functions still use the AST emitter while the IR grows.

Function declarations may omit `fn`: `add(a:i32,b:i32)->i32{a+b}`. `tok compact file.tok` prints this shorter form, and `tok compact --write file.tok` applies it. The [token experiment](research/COMPACT_FUNCTIONS.md) measures the change across 43 executable examples.

Local `let` and `var` bindings can infer complete initializer types. A [versioned benchmark experiment](research/INFERRED_BINDINGS.md) measures the source-token savings and records its limits.

Experimental [filesystem capabilities](spec/FILESYSTEM_CAPABILITY.md) let Tokit read and write UTF-8 files through separate path grants. See [file_copy.tok](examples/file_copy.tok) for a two-grant example.

The same grants also protect `read_bytes` and `write_bytes` for exact binary data. [binary_copy.tok](examples/binary_copy.tok) copies a file even when it is not valid UTF-8.

An experimental typed task primitive lets `spawn f(args)` start a pure function and `join(task)` recover its result. See [the task example](examples/task_square.tok) and [subset contract](spec/EXPERIMENTAL_SUBSET.md).

Experimental file imports let the CLI load relative `.tok` files inside the entry directory. [The multi-file example](examples/modules/main.tok) uses an explicit alias (`import math="math.tok";`), a public declaration (`pub fn triple...`), and a qualified call (`math::triple(7)`). Other declarations stay private to their file. See the [module contract](spec/MODULE_SYSTEM_CANDIDATE.md).

An experimental [local package import](spec/LOCAL_PACKAGE_CANDIDATE.md) can load a `.tok` file or module tree declared with a SHA-256 content pin in `tok.toml` and a matching `tok.lock`. Local packages can declare their own pinned dependencies; the lockfile records the complete graph. [The JSON package example](examples/package_json/main.tok) and [module tree example](examples/package_tree/app/main.tok) demonstrate the provisional `pkg:` syntax.

`tok add <entry.tok> <name> <relative-path> [--entry <relative.tok>]` and `tok rm <entry.tok> <name>` manage local package declarations and refresh `tok.lock`. They preserve comments in `tok.toml`; registry packages and version resolution are not yet supported.

`Option<T>` now provides typed `Some(value)`/`None` values and exhaustive matching; [option_lookup.tok](examples/option_lookup.tok) shows the provisional syntax.

Experimental `Bytes` values store packed byte data, with UTF-8 and checked `[i32]` conversions. [byte_values.tok](examples/byte_values.tok) shows encoding, indexing, and decoding. The earlier pure `utf8_bytes(String)->[i32]` and `utf8_decode([i32])->Option<String>` remain available for existing parser experiments; [utf8_roundtrip.tok](examples/utf8_roundtrip.tok) shows that path.

An experimental [JSON module](spec/JSON_MODULE_CANDIDATE.md) parses and renders recursive JSON values in Tokit. Its parser reports byte offsets and has an explicit nesting limit; packaging and performance work remain open.

Programs can read their own arguments with `args()->[String]`. For example, `tok run examples/arguments.tok -- hello world` and a binary built from that file both print the program arguments without launcher options.

The provisional library also provides `len<T>([T])->i32` and `parse_i32(String)->Result<i32,ParseError>`. [parse_argument.tok](examples/parse_argument.tok) uses both to accept a numeric CLI argument with typed parse errors.

Experimental `i64` values use explicit literals such as `3000000000i64`, checked arithmetic, and `parse_i64(String)->Result<i64,ParseError>`. `i64(value)` widens an `i32`; `i32(value)` narrows an `i64` with an `Option` result. [i64_counter.tok](examples/i64_counter.tok) reads a wide CLI integer. The types do not mix implicitly.

Mutable arrays support `xs.push(value);`; [parse_numbers.tok](examples/parse_numbers.tok) builds an array of checked integers from any number of CLI arguments.

Mutable strings support `text.push(piece);` with a `String` piece. [string_builder.tok](examples/string_builder.tok) demonstrates append without rebuilding the accumulated text on each step.

The experimental `while` statement handles iterative control flow beyond array traversal; [iterative_factorial.tok](examples/iterative_factorial.tok) shows its current syntax.

Both `for` and `while` support loop-local `break;` and `continue;`; [loop_control.tok](examples/loop_control.tok) combines them.
