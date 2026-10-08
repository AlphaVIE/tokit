# Developer tools

| Command | Purpose |
| --- | --- |
| `tok doc [--private] file.tok` | Markdown reference: the file's top comment block, then each public record, enum, and function with its signature and the `//` lines directly above it (`--private` includes private declarations). |
| `tok lint [--json] file.tok` | Advisory warnings: `W001` a `let`/`var` binding is never read, `W002` a `var` is never changed (use `let`), `W003` a private function is never called (`main`, `test_*`, and `bench_*` are entry points). Names starting with `_` are ignored. The analysis is name-based per function, so shadowed names count as used when any binding is read. |
| `tok bench [--iterations N] file.tok` | Times every parameterless `bench_*` function: it replaces `main` with a driver that warms up for N/10 calls, measures N calls with `clock_ns`, and prints `name ns/iter`. The driver is compiled natively; without a Rust toolchain it runs in the interpreter and says so. |
| `tok repl` | Interactive session: declarations accumulate, `let`/`var` lines are remembered and replayed before each evaluation (avoid effects in them), and other lines are evaluated and printed. `:reset` clears the session, `:quit` ends it. |
| `tok expand file.tok` | Readable multi-line Tokit that compiles back to the same program. |
| `tok explain [--pseudo] file.tok` | Structured summary, or pseudocode with `--pseudo`. |

Warnings never fail a build; they use the same `code@line:column message`
format and JSON shape as diagnostics.

## tok run --native

`tok run --native [grants] file.tok [-- args]` compiles the program with the
native backend and runs the executable, passing the same `--allow-read`,
`--allow-write`, and `--allow-net` grants, arguments, standard input, and exit
status. Executables are cached in `$TOK_HOME/cache/native/` (default
`~/.tok`) under a SHA-256 of the generated Rust source and the `rustc -vV`
output, so a second run of an unchanged program skips compilation entirely:
a loop that takes 946 ms in the interpreter takes about 100 ms this way. It
needs a Rust toolchain and cannot be combined with `--json`. The interpreter
remains the reference for semantics; both are tested to agree. Delete the
cache directory to reclaim space.
