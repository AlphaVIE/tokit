# Pointer-chase benchmark review

The [versioned snapshot](../benchmarks_version_04102026_213748/README.md)
revises the earlier [array-cycle benchmark](ARRAY_MATRIX_REVIEW.md). It reads
the next index from a 64-element runtime-supplied array on every iteration.
The permutation is one full cycle, so each load determines the next address.
The matrix runner validates the contract and every program output; a separate
Tokit test uses another runtime-supplied cycle to detect a hard-coded route.

The [raw 40-million-iteration matrix report](results/pointer-chase-matrix-2026-10-04-windows-gnu.json)
keeps five runtime samples, source hashes, full contract, and tool versions.
Local Windows GNU medians for whole-process runtime were:

| Language | Runtime (ms) | Source tokens (`cl100k_base`) |
| --- | ---: | ---: |
| Tokit native | 50.05 | 126 |
| Rust | 50.17 | 229 |
| JavaScript | 95.06 | 127 |
| Python | 1165.78 | 97 |

The [scalar cycle context](results/pointer-chase-scalar-context-2026-10-04-windows-gnu.json)
at 39,999,999 iterations measured 23.79 ms for Tokit and 23.51 ms for Rust.
Those are different programs with different outputs, so the roughly 2.1-fold
gap is directional evidence that the dependent reads add work, not an isolated
array cost. The near-equal Tokit and Rust native medians support parity for
this specific workload on this host; they do not establish general runtime
parity. The 64-element route may remain in cache and does not test large-array
memory behavior.

The local matrix is incomplete: C++, C#, Go, and TypeScript toolchains were
unavailable. CI smoke-checks all eight languages on the same validated
contract. Token counts cover complete source files and are not generation or
repair token costs. The single compilation observation per program is not a
build-time comparison; the Tokit build also performs frontend checking and
Rust generation, whereas the Rust row invokes `rustc` directly.
