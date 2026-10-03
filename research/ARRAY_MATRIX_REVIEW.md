# Versioned array-access benchmark review

The first draft of this fixture embedded `[1,2,3]` directly in every
program. A 99,999,999-iteration trial gave Tokit and Rust almost the same
whole-process runtime as the arithmetic-only cycle. Because the array was
known at compile time, that trial was too weak to distinguish optimized
array work from an equivalent scalar loop. The retained snapshot now reads
the three values as separate runtime arguments. `contract.json` pins the
test values, and the matrix runner computes the expected sum from them.
Every timed output is checked. A separate Tokit test uses `2,5,9` to prove
the program reads the supplied values instead of a baked-in constant.

The [10-million-iteration](results/array-access-matrix-2026-10-04-windows-gnu.json)
and [99,999,999-iteration](results/array-access-matrix-long-2026-10-04-windows-gnu.json)
raw reports keep five runtime samples per available language and source
SHA-256 hashes. The longer Windows GNU run measured these process-runtime
medians in milliseconds:

| Language | Scalar cycle | Runtime-value array cycle |
| --- | ---: | ---: |
| Tokit native | 51.31 | 51.02 |
| Rust | 51.17 | 51.02 |
| JavaScript | 116.70 | 122.49 |
| Python | 3586.01 | 5248.48 |

The Tokit and Rust differences are within the noise of this local run.
The benchmark therefore does not establish an array-access speed advantage
or isolate cache traffic. Source-token counts with `cl100k_base` and
`tiktoken==0.14.0` are 145 for Tokit, 112 for Python, 171 for JavaScript,
and 287 for Rust in the array fixture. These are complete source files,
including command-line parsing. They are not generation/repair token costs.

The Windows report is incomplete: C++, C#, Go, and TypeScript toolchains
were unavailable locally. CI smoke-checks the fixture on its configured
toolchains. Compilation time has one observation per program and is not a
build-time comparison. The Tokit `tok build` command includes frontend
checking and Rust source generation; the Rust row invokes `rustc` directly.
Further work should use a larger runtime-created array and collect CPU time,
allocation counts, and peak memory before drawing conclusions about data
access performance.
