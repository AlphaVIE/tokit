# Packed byte builders in the JSON module

The experimental `Bytes` value now supports `data.push(byte);` on a mutable
binding. The argument is typed as `i32` and checked at runtime to be in
0–255 (`E207`). Both backends preserve value copies and produce the same
diagnostic for an invalid byte. The JSON module uses packed byte buffers
for number lexemes, string parsing, and string escaping. Small temporary
codepoint arrays remain `[i32]`.

The [before](results/json-byte-builders-before-windows-gnu.json) and
[after](results/json-byte-builders-after-windows-gnu.json) reports use the
same Windows GNU host, rustc 1.98.1, corpus, and checked outputs. Each runs
100 parse/render iterations, one warmup, and three timed samples. Build
time has only one observation per side. Medians in milliseconds:

| Input | Interpreter before | Interpreter after | Native before | Native after |
| --- | ---: | ---: | ---: | ---: |
| Record | 188.83 | 185.70 | 9.94 | 9.80 |
| Nested | 1324.45 | 1313.16 | 31.80 | 31.36 |
| Escapes | 980.54 | 966.65 | 13.47 | 12.27 |

These small changes do not establish a general runtime improvement. The
packed representation reduces buffer storage per byte, but this run does
not measure allocations or peak memory. Source tokens with `cl100k_base`
and `tiktoken==0.14.0` fall from 2747 to 2742 after using inferred local
types; UTF-8 bytes rise from 8754 to 8785. The byte-builder API and JSON
module remain experimental.

Reproduce the source comparison by checking out the before revision from
the report's `json.tok` SHA-256, copying `bench_main.tok` and that source
into a directory, and running `scripts/measure_json.py --entry
path/to/bench_main.tok --rust-toolchain 1.98.1 --iterations 100 --samples 3
--build-samples 1`. Repeat with the after revision. The runner's `--entry`
flag keeps each source version isolated while using the same workload.
