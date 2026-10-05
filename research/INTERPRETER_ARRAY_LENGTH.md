# Interpreter array length without a value copy

The reference interpreter evaluated `len(array_variable)` by cloning the
entire array as a function argument. It now borrows the binding while reading
its length. `len` on a computed expression keeps the owned evaluation path.
The same rule applies to `Bytes`. Array copies and mutations retain value
semantics; a parity test covers a copied array, a changed original, and a
computed array expression.

The [raw local report](results/interpreter-array-length-2026-10-05-windows-gnu.json)
uses the new [versioned array-length fixture](../benchmarks_version_05102026_025134/README.md).
Both variants were built with the same Rust 1.98.1 GNU debug profile. The
report pins the benchmark source and interpreter hashes. Each invocation ran
`tok run benchmarks_version_05102026_025134/array_length.tok -- 2000 SIZE`
and checked the exact formula output. One warmup per variant preceded eight
alternating runs in before/after/after/before order, repeated twice.

| Elements | Before ms | After ms | Speedup |
| ---: | ---: | ---: | ---: |
| 4,096 | 273.50 | 23.22 | 11.78× |
| 16,384 | 1,054.54 | 49.20 | 21.43× |

These are whole-process medians, including startup, parsing, checking, and
array construction. The benchmark deliberately repeats `len` on a large
array, so it demonstrates the removed copy cost rather than general program
speed. Native execution is unchanged.
