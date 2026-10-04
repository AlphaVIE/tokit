# JSON escape append experiment

The renderer used to concatenate the accumulated output, decoded plain-text
run, and escape piece whenever it found a byte requiring JSON escaping.
It now appends both pieces to the mutable output and appends the final plain
run and closing quote. The package manifest digest and lockfile were updated
for the changed module bytes.

The [raw alternating-run report](results/json-escape-append-paired-2026-10-04-windows-gnu.json)
contains two runs of each variant, in before/after/after/before order. Each
run rendered every fixture 20 times, checked the exact canonical text on
every iteration, used one warmup, and retained three runtime samples. Pooled
whole-process medians on this Windows GNU host were:

| Case | Runner | Before (ms) | After (ms) |
| --- | --- | ---: | ---: |
| Escape-heavy | Native | 7.72 | 7.27 |
| Escape-heavy | Interpreter | 97.53 | 96.64 |
| Wide document | Native | 38.09 | 37.37 |
| Wide document | Interpreter | 1010.28 | 1011.67 |

The native escape-heavy difference is small and individual samples overlap.
The wide interpreter case did not improve. These runs support a modest local
native improvement, not a general speed claim. The benchmark includes one
parse, value copies, rendering, process startup, and text-length checks.
It does not count allocations or isolate escaping. Compilation has only two
observations per variant and is not analyzed here.

The module grew from 8,781 to 8,819 UTF-8 bytes, from 2,739 to 2,744
`cl100k_base` tokens, and from 2,772 to 2,776 `o200k_base` tokens with
`tiktoken==0.14.0`. This is a performance versus source-token tradeoff,
and the module remains experimental.
