# Boolean syntax token experiment

The experimental grammar adds `!`, `&&`, and `||` for checked `bool`
expressions. `&&` and `||` evaluate the right operand only when needed.
The three pairs below have the same result for every boolean argument;
the branch version uses the previous available syntax. The
[raw counts](results/boolean_logic_tokens.json) retain each source string,
pinned tokenizer versions, and the Qwen tokenizer hash.

| Expression | Branch bytes → compact bytes | cl100k_base | o200k_base | Qwen2.5-Coder |
| --- | ---: | ---: | ---: | ---: |
| `a&&b` | 42 → 28 | 18 → 14 | 18 → 14 | 18 → 14 |
| `a||b` | 41 → 28 | 18 → 14 | 18 → 14 | 18 → 14 |
| `!a` | 38 → 19 | 15 → 10 | 15 → 10 | 15 → 10 |

In a representative `if a&&b{1}else{0}` snippet, replacing `&&` with a
single `&` saved one source byte but no tokens in any of the three measured
tokenizers. The double operator makes short-circuit behavior explicit and
leaves single `&` and `|` available for future uses. This is a local syntax
measurement, not evidence about total generation or repair cost.

```text
.venv/Scripts/python.exe scripts/measure_boolean_logic_tokens.py --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json --output research/results/boolean_logic_tokens.json
```
