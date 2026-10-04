# Shared packed byte values

`Bytes` keeps value semantics while its implementation now shares immutable
buffers between copies. A `push` uses copy-on-write, so changing one value
cannot change a previous copy. The interpreter and native backend both use
`Arc<Vec<u8>>` in both backends, because task values can cross threads.
Native conversions that need an owned `Vec<u8>` reclaim a uniquely owned
buffer and copy only when another value still shares it.

The JSON benchmark now includes a 6,494-byte wide document with 128 events.
The fixture checks parse/render output on every run. The [main baseline](results/json-shared-bytes-before-2026-10-04-windows-gnu.json)
and [candidate report](results/json-shared-bytes-after-2026-10-04-windows-gnu.json)
retain source hashes, toolchain, host, build sizes, and timed samples. Both
use five iterations, one warmup, and three measured runs per fixture. The
small fixtures show only minor differences.

To reduce order effects, the wide document was also run through old and new
interpreter and native binaries in alternating order for nine samples each.
All 36 runs produced `Some(32470)`. Times include process startup (ms):

| Runner | Samples | Median |
| --- | --- | ---: |
| Interpreter before | 739.84, 728.50, 729.91, 729.61, 730.79, 727.04, 728.37, 732.75, 731.41 | 729.91 |
| Interpreter after | 709.37, 709.72, 703.89, 698.86, 704.67, 701.19, 704.20, 703.86, 706.01 | 704.20 |
| Native before | 106.58, 25.00, 25.22, 25.15, 25.81, 24.99, 25.37, 24.66, 25.48 | 25.22 |
| Native after | 99.03, 22.07, 22.69, 22.20, 22.74, 21.71, 22.45, 22.12, 22.18 | 22.20 |

The old compiler binary came from main commit `fb4b3b8`; both versions used
the same Tokit JSON sources. On this Windows x86-64 GNU host, the wide-case
medians improve by 3.5% for the interpreter and 12.0% for the native binary.
The first native sample on each binary includes startup outliers, so medians
are more representative than means. These measurements do not isolate parser
allocation counts, generalize to larger files, or establish a build-time
improvement.
