# Compact integer type names

Tokit accepts `I` for `i32` and `L` for `i64` in parsed type positions.
`tok compact --types-only file.tok` makes that substitution without changing
identifiers in expressions, conversion calls, literal suffixes, comments, or
strings. Plain `tok compact` also removes optional `fn` declaration keywords.
These spellings are experimental; `I` and `L` are reserved in type positions.

The measurements cover the same 43 `examples/*.tok` files and 101 function
declarations as the [function keyword experiment](COMPACT_FUNCTIONS.md). The
script checks that each output parses, formats, and is idempotent. Original
source hashes, compiler hash, pinned tokenizer versions, and per-file counts
are in the [type-only](results/compact_integer_types.json) and
[combined](results/compact_combined.json) result files.

| Metric | Original | Types only | Combined | Combined saved |
| --- | ---: | ---: | ---: | ---: |
| UTF-8 bytes | 20,270 | 19,996 | 19,693 | 577 |
| cl100k_base tokens | 6,304 | 6,167 | 6,068 | 236 (3.7%) |
| o200k_base tokens | 6,374 | 6,237 | 6,141 | 233 (3.7%) |
| Qwen2.5-Coder tokens | 6,731 | 6,457 | 6,358 | 373 (5.5%) |

Build the debug CLI, then run:

```text
.venv/Scripts/python.exe scripts/measure_compact_functions.py --mode types-only --examples examples --tok target/debug/tok.exe --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json --output research/results/compact_integer_types.json
.venv/Scripts/python.exe scripts/measure_compact_functions.py --mode all --examples examples --tok target/debug/tok.exe --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json --output research/results/compact_combined.json
```

This measures source encoding, not total agent cost or generation accuracy.
Literal suffixes and library calls remain long spellings, so their token costs
are unchanged. The syntax may change after broader usability tests.
