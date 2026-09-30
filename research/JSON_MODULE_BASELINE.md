# JSON module seed measurements

The formatted [Tokit JSON module](../examples/json/json.tok) is a workload
seed for future equivalent-language comparisons. Measured with
scripts/token_cost.py, tiktoken 0.14.0, and the checked tok stats command:

| Stage | UTF-8 bytes | AST nodes | Semantic operations | cl100k_base | o200k_base |
| --- | ---: | ---: | ---: | ---: | ---: |
| Renderer only (`a688589`) | 2599 | 540 | 165 | 836 | 852 |
| Parser and renderer | 8750 | 1757 | 628 | 2773 | 2810 |

These counts cover the module alone. They omit the importing entry file,
tests, prompts, compiler feedback, and repairs. They do not establish
token efficiency against another language or model family. The parser
copies byte arrays between calls, while the renderer copies arrays and
repeatedly concatenates strings. Allocation behavior remains unmeasured.
Add a semantically equivalent corpus in other languages before any
cross-language performance claim.

## Local parser and renderer measurements

`scripts/measure_json.py` builds [bench_main.tok](../examples/json/bench_main.tok)
and measures three documents: an 81-byte record with an exact exponent
lexeme, a 608-byte nested collection, and a 684-byte escaped Unicode
document. Each process parses and renders its input 20 times, then returns
the number of rendered UTF-8 bytes. Expected byte counts are defined
independently in the Python fixture. The runner checks both interpreter
and native output before recording elapsed time.

Observed on Windows x86-64 with rustc 1.98.1 GNU target and Python 3.11.9,
one warmup and five timed samples per runner. Build medians use three
sequential builds with a warm OS/toolchain cache. Times are wall clock and
include process startup. The native bootstrap compiles generated Rust at
`-C opt-level=2`.

| Document | Interpreter median | Native median |
| --- | ---: | ---: |
| Record, 81 input bytes | 63.86 ms | 7.81 ms |
| Nested, 608 input bytes | 1289.21 ms | 13.12 ms |
| Escapes, 684 input bytes | 830.58 ms | 8.90 ms |

The native build median was 1037.82 ms and the resulting executable was
5,043,580 bytes. These measurements reveal a large interpreter cost for
the larger documents. They do not isolate parsing from rendering, startup,
or allocation, and do not support a cross-language performance claim.
Repeat with an equivalent external implementation and a more rigorous
performance harness before making one. To reproduce this host's run:

```powershell
python scripts/measure_json.py --rust-toolchain stable-x86_64-pc-windows-gnu --output research/results/json-windows-gnu.json
```
