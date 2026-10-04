# JSON render phase baseline

The existing JSON workload parses and renders on every iteration. The new
[`render_bench_main.tok`](../examples/json/render_bench_main.tok) parses its
argument once, then renders the same value repeatedly. This isolates the
render-heavy path more closely while still including one parse, value copies,
UTF-8 length calculation, and process startup. It makes no allocation or
asymptotic-complexity claim.

The [raw Windows GNU report](results/json-render-baseline-2026-10-04-windows-gnu.json)
uses `scripts/measure_json.py --entry examples/json/render_bench_main.tok
--iterations 20 --warmups 1 --samples 3 --build-samples 2`. The runner checks
every execution against the expected byte count. The 6,494-byte wide JSON
case has a median of 1,035.04 ms in the interpreter and 42.02 ms natively;
the warm native build median is 1,065.81 ms. These are local wall-clock times
on rustc 1.98.1, including startup.

The timed workload checks output length. The separate JSON corpus tests check
exact parser and renderer output, including escapes, ordering, and errors.
Future renderer experiments should compare against this snapshot and retain
both kinds of verification.
