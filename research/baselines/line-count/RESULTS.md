# Line-count executable baseline

The [contract](contract.json) defines eight cases for a valid UTF-8 file or a missing path. Empty input has zero lines; an LF contributes one line, and nonempty input without a trailing LF contributes one final line. CRLF has the same count as LF, while a bare CR does not split a line. The contract excludes invalid UTF-8, non-`NotFound` I/O failures, and counts exceeding `i32`. Tokit's runtime covers invalid UTF-8 separately, but this baseline cannot claim cross-language equivalence for it.

On the Windows development machine, Tokit, Rust, Python, TypeScript, and Java passed all eight cases. Go, C, and C++ were unavailable locally. The CI baseline job uses `--require-all` to verify all eight implementations on Linux.

The source between `BENCH_START` and `BENCH_END` contains the core function. Measured on 2026-09-29 with `tiktoken==0.14.0`, `tokenizers==0.23.2`, and the pinned Qwen tokenizer documented in [the cross-family report](../../CROSS_FAMILY_RESULTS.md):

| Language | Core bytes | cl100k_base | o200k_base | Qwen2.5-Coder |
| --- | ---: | ---: | ---: | ---: |
| Tokit | 168 | 46 | 47 | 48 |
| Rust | 149 | 44 | 44 | 46 |
| Go | 254 | 79 | 80 | 81 |
| C | 514 | 158 | 158 | 160 |
| C++ | 574 | 163 | 162 | 165 |
| Python | 159 | 44 | 44 | 44 |
| TypeScript | 185 | 50 | 52 | 50 |
| Java | 330 | 84 | 88 | 84 |

Reproduce with:

```text
python scripts/verify_line_count.py --require-all
python scripts/measure_line_count.py --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json
```

Tokit is not the smallest core function in this fixture. The implementations have equivalent tested outputs but different internal work: Tokit constructs an array of strings, Rust uses an iterator, and several languages count LF bytes or code units directly. These measurements exclude imports and command-line wrappers. They do not measure runtime performance, full-program token cost, model generation, or repair success and cannot establish language superiority.
