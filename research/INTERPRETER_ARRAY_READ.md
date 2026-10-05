# Interpreter array read without a full value copy

The reference interpreter copied an entire `[T]` binding before evaluating
`array[index]`. It now borrows a variable binding during the read and clones
only the selected element. Indexing a computed array expression retains the
existing owned evaluation path. Array assignment and `push` still have value
semantics; a regression test checks a copied array, a mutated original, and a
computed array expression against the native backend.

The [raw local report](results/interpreter-array-read-2026-10-05-windows-gnu.json)
uses the existing [generated-array chase](../benchmarks_version_04102026_220333/README.md)
source. Both variants were built with the same Rust 1.98.1 GNU debug profile.
The report pins the source and interpreter hashes. Each invocation ran
`tok run benchmarks_version_04102026_220333/generated_chase.tok -- 2000 SIZE 13`
and checked the exact oracle output. One warmup per variant preceded eight
alternating runs in before/after/after/before order, repeated twice.

| Elements | Before ms | After ms | Speedup |
| ---: | ---: | ---: | ---: |
| 4,096 | 290.50 | 37.96 | 7.65× |
| 16,384 | 1,086.40 | 92.57 | 11.74× |

These medians include process startup, parsing, type checking, and array
construction. The `2,000` timed reads copy increasingly large arrays in the
old interpreter, so this is a targeted stress case rather than a general
language speed claim. Native execution is unchanged.
