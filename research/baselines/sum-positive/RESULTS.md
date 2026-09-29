# Sum-positive executable baseline

The [contract](contract.json) specifies five bounded input/output cases. The verifier compiles or runs each language and compares actual stdout with the expected result. On the Windows development machine, Tokit, Rust, Python, TypeScript, and Java passed all five cases. C, C++, and Go were unavailable locally. The [CI baseline run](https://github.com/AlphaVIE/tokit/actions/runs/36504796041) verified all eight languages against all five cases and reported `skipped: []`.

The source region between `BENCH_START` and `BENCH_END` contains each core function, excluding argument parsing and output wrappers. Measured on 2026-09-29 with `tiktoken==0.14.0`, `tokenizers==0.23.2`, and the pinned Qwen tokenizer documented in [the cross-family report](../../CROSS_FAMILY_RESULTS.md):

| Language | Core bytes | cl100k_base | o200k_base | Qwen2.5-Coder |
| --- | ---: | ---: | ---: | ---: |
| Tokit | 106 | 42 | 42 | 45 |
| Rust | 130 | 46 | 46 | 48 |
| Go | 141 | 43 | 43 | 46 |
| C | 181 | 66 | 66 | 69 |
| C++ | 162 | 55 | 55 | 59 |
| Python | 129 | 39 | 39 | 39 |
| TypeScript | 147 | 44 | 44 | 44 |
| Java | 162 | 46 | 46 | 46 |

Reproduce the source-only counts with:

```text
python scripts/fetch_qwen_tokenizer.py
python scripts/measure_sum_positive.py --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json
```

This single core function is cheaper in Tokit than Rust, Go, C, C++, TypeScript, and Java under the two OpenAI encodings, but Python is cheaper. The gaps to Go and TypeScript are small. The measured Tokit function is an experimental, runnable subset and differs from the earlier hand-written A notation. The trial does not measure full programs, generation accuracy, repair cost, compiler speed, or runtime performance. Bounded cases do not prove identical overflow behavior. The evidence is therefore insufficient to select a language grammar or claim Tokit meets its token-efficiency target.
