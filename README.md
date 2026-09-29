# Tokit

Tokit explores a machine-first programming language optimized for the total cost of producing, checking, and repairing programs with language models. The language design is experimental; no source syntax has been selected yet.

The project brief is in [PROMPT.txt](PROMPT.txt). Start with [VISION.md](VISION.md), [DESIGN_GOALS.md](DESIGN_GOALS.md), and [RESEARCH_PLAN.md](RESEARCH_PLAN.md). The [candidate comparison](LANGUAGE_CANDIDATES.md) and [initial measurements](research/INITIAL_RESULTS.md) are experiments, not a frozen syntax.

An [experimental candidate A compiler slice](spec/EXPERIMENTAL_SUBSET.md) can parse, type-check, interpret, explain, [format source](spec/FORMATTER.md), [measure structural density](research/STRUCTURAL_METRICS.md), and [build native executables](spec/NATIVE_BOOTSTRAP.md) from a small `.tok` subset. With Rust installed, run `cargo test --workspace`, `cargo run -p tokit-compiler --bin tok -- run examples/answer.tok`, or `cargo run -p tokit-compiler --bin tok -- explain examples/match_result.tok`. The source grammar and bootstrap backend are not yet production language commitments.

An [experimental filesystem-read capability](spec/FILESYSTEM_CAPABILITY.md) lets `tok run --allow-read examples examples/line_count.tok` process UTF-8 files with explicit access to a path.

An experimental typed task primitive lets `spawn f(args)` start a pure function and `join(task)` recover its result. See [the task example](examples/task_square.tok) and [subset contract](spec/EXPERIMENTAL_SUBSET.md).

`Option<T>` now provides typed `Some(value)`/`None` values and exhaustive matching; [option_lookup.tok](examples/option_lookup.tok) shows the provisional syntax.

Programs can read their own arguments with `args()->[String]`. For example, `tok run examples/arguments.tok -- hello world` and a binary built from that file both print the program arguments without launcher options.

The provisional library also provides `len<T>([T])->i32` and `parse_i32(String)->Result<i32,ParseError>`. [parse_argument.tok](examples/parse_argument.tok) uses both to accept a numeric CLI argument with typed parse errors.

Mutable arrays support `xs.push(value);`; [parse_numbers.tok](examples/parse_numbers.tok) builds an array of checked integers from any number of CLI arguments.

The experimental `while` statement handles iterative control flow beyond array traversal; [iterative_factorial.tok](examples/iterative_factorial.tok) shows its current syntax.

Both `for` and `while` support loop-local `break;` and `continue;`; [loop_control.tok](examples/loop_control.tok) combines them.
