# Explicit `i64` literal experiment

Tokit now keeps unsuffixed integers as `i32` and uses an adjacent `i64`
suffix for wide literals. `-9223372036854775808i64` is one signed wide
literal; unary negation of a computed minimum still fails with `E201`.
There is no implicit conversion or mixed arithmetic between `i32` and
`i64`. `i64(value)` widens an `i32` without loss; `i32(value)` returns
`Option<i32>` when narrowing an `i64`. This keeps type inference and
overflow behavior explicit.

With `tiktoken==0.14.0`, both `cl100k_base` and `o200k_base` give these
token counts for a representative small value:

| Spelling | Tokens |
| --- | ---: |
| `100L` | 2 |
| `100i64` | 3 |
| `i64(100)` | 5 |

The `L` suffix saved one token in this snippet. `i64` was chosen because it
names the exact declared type and extends consistently to future sized
integer types. These counts are isolated source snippets, not generation or
repair experiments; no model error-rate comparison has been run. The
syntax remains experimental.
