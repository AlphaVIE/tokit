# Wide-integer benchmark snapshot

Snapshot: `benchmarks_version_05102026_022806`. The primary contract cycles
through 64 positive runtime values above `i32::MAX` for 40,000,000 iterations.
The independent oracle expects `Ok(120000021420000000)`. The alternate
four-value contract is checked separately with 3,000 iterations. CI checks
both contracts in all eight languages.

The local Windows GNU run used one warmup and five timed samples per language.
Each runtime is the median whole-process elapsed time, including startup and
argument parsing. Source token counts use `cl100k_base` (tiktoken 0.14.0) and
cover the entire program.

| Language | Runtime ms | Source tokens | Status |
| --- | ---: | ---: | --- |
| C++ | — | 224 | Toolchain unavailable locally |
| C# | — | 128 | SDK 8+ unavailable locally |
| Go | — | 205 | Toolchain unavailable locally |
| JavaScript | 233.38 | 146 | Output verified |
| Python | 2046.85 | 111 | Output verified |
| Rust | 34.37 | 189 | Output verified |
| Tokit | 34.21 | 147 | Output verified |
| TypeScript | — | 161 | Toolchain unavailable locally |

The raw samples, source hashes, tool versions, contract, and output statuses
are in `results/i64-matrix-2026-10-05-windows-gnu.json`. These values describe
this machine and workload; they do not isolate integer addition from array
access or process startup. JavaScript and TypeScript use `BigInt` to preserve
the exact result.
