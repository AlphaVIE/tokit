# Checked-division executable baseline

The [contract](contract.json) defines seven input/output cases for signed `i32` division with truncation toward zero and a typed zero-divisor error. `i32::MIN / -1` is excluded because its overflow policy is separate. The source region between `BENCH_START` and `BENCH_END` contains the core type and function definitions; command-line and output adapters are excluded. The eight implementations use each language's available tagged-result representation, so their static guarantees differ.

On the Windows development machine, Tokit, Rust, Java, Python, and TypeScript passed all seven cases. C, C++, and Go were unavailable locally; CI runs all eight with `--require-all`. The current CI result should be linked here after it completes.

Measured on 2026-09-29 with `tiktoken==0.14.0`, `tokenizers==0.23.2`, and the pinned Qwen tokenizer described in [the cross-family report](../../CROSS_FAMILY_RESULTS.md):

| Language | Core bytes | cl100k_base | o200k_base | Qwen2.5-Coder |
| --- | ---: | ---: | ---: | ---: |
| Tokit | 118 | 44 | 44 | 47 |
| Rust | 141 | 53 | 53 | 56 |
| Go | 230 | 70 | 70 | 72 |
| C | 247 | 83 | 83 | 86 |
| C++ | 158 | 52 | 52 | 55 |
| Python | 248 | 82 | 82 | 82 |
| TypeScript | 261 | 82 | 82 | 82 |
| Java | 306 | 75 | 75 | 75 |

Reproduce the bounded semantic checks and source-only counts with:

```text
python scripts/verify_checked_division.py --require-all
python scripts/fetch_qwen_tokenizer.py
python scripts/measure_checked_division.py --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json
```

For this one core region, Tokit uses fewer tokens than the seven comparison languages under all three encodings. This does not establish a general token advantage: it excludes adapters, imports, downstream edits, generation errors, repair cost, runtime speed, and overflow behavior. The syntax is still experimental.
