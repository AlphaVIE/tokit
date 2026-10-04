# Generated-array chase: size and stride

The earlier [64-element pointer chase](POINTER_CHASE_MATRIX.md) made reads
data-dependent, but its working set was small. The new
[versioned snapshot](../benchmarks_version_04102026_220333/README.md)
builds its array from runtime size and stride arguments. Four validated
contracts use the same eight source files. The runner checks exact output
on every execution, and a Tokit test changes runtime size and stride.

The [raw four-case report](results/generated-array-chase-2x2-2026-10-04-windows-gnu.json)
contains five timed samples per available language and case, source hashes,
token counts, tool versions, and contracts. Whole-process medians in
milliseconds on one Windows GNU host were:

| Elements | Stride | Tokit native | Rust | JavaScript | Python |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 16,384 | 13 | 54.20 | 53.16 | 109.13 | 1183.70 |
| 262,144 | 13 | 57.36 | 55.79 | 118.24 | 1210.23 |
| 16,384 | 8,191 | 52.19 | 51.55 | 97.90 | 1194.07 |
| 262,144 | 8,191 | 103.25 | 101.43 | 180.24 | 1464.45 |

The large, wide-stride case is about twice as slow as the small case for
Tokit and Rust. With stride 13, changing the size produced little difference
in these local native runs. This interaction is consistent with a memory
access effect, but **cache misses were not measured**. Array construction,
bounds checks, threshold branches, and process startup remain in every
runtime. Size also changes the wrap pattern and the exact output. These
results do not establish general language speed rankings.

The Tokit source has 170 `cl100k_base` tokens, compared with 238 for Rust,
169 for JavaScript, and 138 for Python (`tiktoken==0.14.0`). These counts
cover complete programs and do not measure generation or repair cost.
Compilation has only one observation per program in each case and is not
a build-speed comparison. C++, C#, Go, and TypeScript toolchains were absent
locally; CI smoke-checks all eight languages with three contracts. The
library and compiler remain experimental.
