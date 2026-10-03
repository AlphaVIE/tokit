# Cycle-sum snapshot with inferred local bindings

This version keeps the exact cycle-sum contract and the seven comparison
implementations from [the first snapshot](../benchmarks_version_02102026_014800/README.md).
Only `cycle_sum.tok` changes: local `let` and `var` bindings omit type
annotations when the initializer determines a complete type. Function
interfaces and error behavior are unchanged.

This is a paired source-token experiment, not a new runtime workload. The
original Tokit file is 312 bytes and 116 `cl100k_base` tokens; this version
is 287 bytes and 105 tokens with `tiktoken==0.14.0`. The 11-token reduction
does not make Tokit shorter than the Python baseline's 89 tokens for this
fixture. The compiler rejects ambiguous inferred values such as `[]`,
`None`, and `Ok(1)` until an annotation supplies the missing type.

Run the matrix for both snapshots to verify output and collect paired
runtime samples:

```sh
python scripts/benchmark_matrix.py --iterations 300000 --warmups 1 --samples 5 --output result.json
```
