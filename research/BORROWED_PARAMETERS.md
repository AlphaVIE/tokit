# Lending non-scalar parameters in the native bootstrap

Tokit parameters are immutable (`E109` rejects assignment and `push`). The
native bootstrap therefore passes every parameter whose declared type is not
`i32`, `i64`, `f64`, `bool`, `Unit`, or a bare type parameter by shared Rust
reference. Before this change, each call cloned the argument, for example a
whole array, string, or record, even if the callee only read one element.

Value semantics are unchanged: every read of a lent parameter inside the
callee still produces its own copy, and lent values cannot be mutated. Bare
type parameters (`x:T`) stay by value because they may be instantiated with
scalars. Spawned tasks own their argument and lend it to the task function.
The reference interpreter is unchanged.

## Measurement

[`scripts/benchmark_borrowed_params.py`](../scripts/benchmark_borrowed_params.py)
builds each workload with a baseline and a candidate CLI and requires
identical output for every run. The [raw report](results/borrowed-params-2026-10-06-windows-gnu.json)
records compiler, fixture hashes, and all samples. Both CLIs were release
builds with Rust 1.98.1 GNU on Windows; the baseline was the parent commit.
Times are whole-process medians of seven runs.

| Workload | Before ms | After ms | Ratio |
| --- | ---: | ---: | ---: |
| [Array argument](../benchmarks/borrowed_params/array_arg.tok), 20,000 calls on 200,000 elements | 658.7 | 15.6 | 42.22× |
| [Record argument](../benchmarks/borrowed_params/record_arg.tok), 20,000 calls | 2,672.9 | 1,317.5 | 2.03× |
| JSON parse and render, 300 rounds | 338.5 | 286.7 | 1.18× |
| JSON render, 3,000 rounds | 1,813.6 | 1,356.1 | 1.34× |

The array fixture is deliberately the best case. The record fixture remains
slow because `len(d.sizes)` still copies the projected field; borrowing field
projections inside builtin calls is a separate follow-up. Source tokens are
unchanged.
