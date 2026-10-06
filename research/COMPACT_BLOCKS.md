# Optional `else` and block statements without `;`

Before this change, every `if` needed an `else` branch, and an `if`, `match`,
or block used as a statement needed a trailing `;`. Imperative Tokit code
therefore contained many `if c{...}else{};` forms; the JSON module alone
had dozens. An `if` without `else` now behaves as `else{}` and requires a
`Unit` (or diverging) branch. Block-like statements end at `}` unless they
are the block's final expression. `tok compact` rewrites existing code to the
shorter form, so both spellings converge on one canonical form.

`tok compact --blocks-only` removes `else{}` only when it is empty and has no
comment. It drops `;` only after an `if` chain or block whose branches have
no final value, and only when the next token is an identifier, a statement
keyword, `if`, `match`, or `}`. A token such as `-` or `[` could continue an
expression, so the `;` stays before it.

## Measurement

The [blocks-only](results/compact_blocks.json) and
[combined](results/compact_combined_blocks.json) result files list per-file
hashes and counts for the 45 `examples/**/*.tok` files. They were produced
with `scripts/measure_compact_functions.py --mode blocks-only` and
`--mode all`, which verify that each output parses and is idempotent. A
separate run compiled and ran every example before and after
`--blocks-only`; all outputs were identical except the content-pinned
`package_json` example, whose dependency hash changes by design.

| Metric | Source | Blocks only | Saved | All compact rules | Saved |
| --- | ---: | ---: | ---: | ---: | ---: |
| UTF-8 bytes | 20,486 | 20,088 | 398 (1.9%) | 19,511 | 975 (4.8%) |
| cl100k_base tokens | 6,382 | 6,191 | 191 (3.0%) | 5,955 | 427 (6.7%) |
| o200k_base tokens | 6,454 | 6,265 | 189 (2.9%) | 6,032 | 422 (6.5%) |
| Qwen2.5-Coder tokens | 6,816 | 6,625 | 191 (2.8%) | 6,252 | 564 (8.3%) |

Savings concentrate in imperative code: most come from `examples/json`.
Expression-oriented examples are unchanged. This measures source encoding,
not generation accuracy. An earlier estimate also tested dropping `->`
before return types; it saved 3 tokens in total because `)->` is already
a single token in all three tokenizers, so it was not pursued.
