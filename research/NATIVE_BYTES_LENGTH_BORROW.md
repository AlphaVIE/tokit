# Native byte-length borrowing

The native backend now borrows the packed byte buffer directly for
`len(data)` when `data` is a `Bytes` variable. It previously emitted
`len(&data.clone().0)`, creating an extra `Arc` reference before the length
check. Other expressions keep their previous evaluation path. The language's
value-copy semantics are unchanged; a generated-code test checks the borrow,
and the native parity test reads the value before and after mutation.

The [raw local before/after report](results/native-bytes-length-borrow-2026-10-04-windows-gnu.json)
pins the JSON fixture and native backend source hashes. Each variant used
ten parse/render iterations, one warmup, and three timed samples. The wide
document's native whole-process median was 33.70 ms before and 33.68 ms
after; the nested case was 9.23 and 9.04 ms. These differences are too small
to establish a workload-level speedup. Interpreter timings are unaffected by
the native code change and were also similar. The benchmark includes parsing,
rendering, and process startup; it does not isolate `len(Bytes)` or count
reference-count operations. The generated code removes one such operation
from this call site, regardless of whether the optimizer would have removed
it later.
