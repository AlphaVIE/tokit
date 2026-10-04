# JSON array and object append experiment

The JSON module now appends array and object members to a mutable `String`.
The escape path is unchanged. A final closing bracket or brace uses one
concatenation outside the loop. Object member text is assembled before it is
appended, so the growing output is not copied for every member. The JSON
package's content pin and lockfile were refreshed for the source change.

All benchmark runs compare each rendered document with its exact canonical
text before accepting the byte-count result. The [before](results/json-render-append-before-2026-10-04-windows-gnu.json)
and [after](results/json-render-append-after-2026-10-04-windows-gnu.json)
reports use 20 render iterations, one warmup, three samples, and two warm
build samples on Windows x86-64 GNU with rustc 1.98.1. The wide document has
128 events and 6,494 input bytes. Nine additional alternating runs per
variant are retained in the [paired report](results/json-render-append-paired-2026-10-04-windows-gnu.json):

| Wide render phase | Before | After | Change |
| --- | ---: | ---: | ---: |
| Interpreter median | 1,016.64 ms | 1,000.12 ms | -1.6% |
| Native median | 42.62 ms | 37.30 ms | -12.5% |

The combined parse/render benchmark also passed exact-output checks. Its
[before](results/json-combined-append-before-2026-10-04-windows-gnu.json)
and [after](results/json-combined-append-after-2026-10-04-windows-gnu.json)
reports use five iterations and three samples; wide-case medians were
699.39 to 693.27 ms in the interpreter and 23.38 to 22.27 ms natively.
Those smaller differences should not be generalized beyond this local run.

The Tokit module changed from 8,785 to 8,781 UTF-8 bytes and from 2,742 to
2,739 `cl100k_base` tokens; `o200k_base` fell from 2,779 to 2,772 tokens
with `tiktoken==0.14.0`. The exact source hashes are in the reports. Build
times varied across two samples and do not establish a build-speed change.
Neither benchmark isolates allocations, parser cost, value copies, or process
startup. The module remains experimental.
