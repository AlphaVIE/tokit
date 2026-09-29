# Generic-pair executable baseline

The [contract](contract.json) constructs a `Pair<T>` and returns a fresh pair with its two fields reversed. Four cases cover `i32`, `bool`, and `String`. The C implementation uses a macro instantiated for those three types; it does not provide open-ended parametric polymorphism. Other languages use their native generic facilities. The marked core region includes definitions but excludes input/output adapters.

On the Windows development machine, Tokit, Rust, Java, Python, and TypeScript passed all four cases. C, C++, and Go were unavailable locally. CI verifies all eight languages with `--require-all`. The string case is ASCII because Windows Java process arguments did not preserve a non-ASCII trial value; this contract does not establish Unicode interoperability. Tokit's separate UTF-8 tests cover direct Unicode literals and native output.

Measured on 2026-09-29 with `tiktoken==0.14.0`, `tokenizers==0.23.2`, and the pinned Qwen tokenizer described in [the cross-family report](../../CROSS_FAMILY_RESULTS.md):

| Language | Core bytes | cl100k_base | o200k_base | Qwen2.5-Coder |
| --- | ---: | ---: | ---: | ---: |
| Tokit | 84 | 28 | 28 | 28 |
| Rust | 113 | 40 | 40 | 40 |
| Go | 114 | 36 | 36 | 36 |
| C | 251 | 72 | 72 | 73 |
| C++ | 122 | 37 | 37 | 37 |
| Python | 157 | 50 | 48 | 50 |
| TypeScript | 121 | 40 | 40 | 40 |
| Java | 122 | 39 | 39 | 39 |

Reproduce with:

```text
python scripts/verify_generic_pair.py --require-all
python scripts/fetch_qwen_tokenizer.py
python scripts/measure_generic_pair.py --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json
```

Tokit has the lowest core-source count for this fixture under these three tokenizers. This does not measure full-program cost, model generation, repair, compile time, runtime speed, or safety equivalence. The syntax is experimental.
