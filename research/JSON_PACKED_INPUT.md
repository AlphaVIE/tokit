# JSON parser input: packed bytes experiment

The JSON parser previously represented the entire UTF-8 input as `[i32]`.
This experiment switches the input passed among parser functions to the
experimental `Bytes` type. Number lexeme and escaped-string construction
still use `[i32]` buffers, and parser calls still copy their input value.
The corpus, grammar, errors, and recursion limit are unchanged.

The [before](results/json-packed-before-windows-gnu.json) and
[after](results/json-packed-after-windows-gnu.json) raw reports include
source hashes, toolchain, host, individual samples, and checked output.
Both used `scripts/measure_json.py` with 20 parse/render iterations,
one warmup, three timed samples, and two build samples. Runs were made
consecutively on the same Windows x86-64 host with rustc 1.98.1 GNU.
Times include process startup; medians below are milliseconds.

| Input | Interpreter before | Interpreter after | Native before | Native after |
| --- | ---: | ---: | ---: | ---: |
| Record, 81 bytes | 64.26 | 47.50 | 7.70 | 7.89 |
| Nested, 608 bytes | 1286.33 | 270.38 | 13.71 | 12.84 |
| Escapes, 684 bytes | 831.82 | 204.30 | 8.84 | 9.19 |

The interpreter improvement is large in these fixtures, especially where
recursive calls used to clone arrays of tagged integer values. Native
samples are close enough that this run does not establish a native speedup.
Build medians were 1056.79 and 1063.39 ms; binary sizes were 5,043,557
and 5,043,688 bytes. This is a local paired observation, not an isolated
measurement of allocations or a cross-language performance claim.
Rendering and process startup are included in every run.

Reproduce each side by checking out a revision whose `json.tok` SHA-256
matches that side's raw report and running:

```powershell
python scripts/measure_json.py --rust-toolchain stable-x86_64-pc-windows-gnu --samples 3 --build-samples 2 --output result.json
```
