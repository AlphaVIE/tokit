# Function declaration compression

Tokit now accepts both `fn add(a:i32)->i32{a}` and
`add(a:i32)->i32{a}`. `pub` and generic parameters work in both forms.
`tok compact file.tok` removes the optional keyword from declarations while
preserving other source bytes; `--write` updates the file. This is a syntax
variant of the same checked functions, not a change to value or call semantics.

The measurement ran `tok compact` over all 43 tracked `.tok` files under
`examples/` at commit `e5d158d`. It verifies the compact output parses,
formats, and is idempotent. The corpus contains 101 function declarations.
Pinned `tiktoken` and Qwen tokenizers are the same as in the earlier
[cross-family experiment](CROSS_FAMILY_RESULTS.md). Full counts and source
hashes are in [the result file](results/compact_functions.json).

| Metric | Original | Compact | Saved |
| --- | ---: | ---: | ---: |
| UTF-8 bytes | 20,270 | 19,967 | 303 |
| cl100k_base tokens | 6,304 | 6,205 | 99 |
| o200k_base tokens | 6,374 | 6,278 | 96 |
| Qwen2.5-Coder tokens | 6,731 | 6,632 | 99 |

```text
.venv/Scripts/python.exe scripts/measure_compact_functions.py --examples examples --tok target/debug/tok.exe --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json --output research/results/compact_functions.json
```

The result is a 1.4–1.6% source-token reduction in this corpus. It measures
source encoding only. It does not establish lower total agent cost, generation
accuracy, parser recovery quality, or language-wide superiority. The older
syntax remains accepted while the experimental grammar is evaluated.
