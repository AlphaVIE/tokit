# Native runtime section pruning

The native backend previously emitted every runtime helper into every generated
Rust program. The emitter now selects the Bytes, I/O, parsing, UTF-8, and task
sections from the checked program's generated Rust body. The core runtime and
argument/grant setup remain unconditional. This changes generated Rust size and
build time, not Tokit source syntax or its optimization level.

On Windows x86-64 GNU with rustc 1.98.1, the `array-cycle` workload from the
versioned benchmark snapshot had a median `tok build` time of 498.6 ms before
and 475.3 ms after (5 and 7 samples respectively, one warmup each). Direct
Rust build times in the same runs were 300.4 ms and 302.3 ms. The Tokit build
time reduction was 23.3 ms (4.7%). The samples are small and were collected
sequentially; treat this as a local result, not a cross-platform speed claim.
The workload output was checked on every build/run by `bench_native.py`.

Raw samples, tool versions, source hashes, binary sizes, and run timings:

- [Before](results/native-build-pruning-before-2026-10-04-windows-gnu.json)
- [After](results/native-build-pruning-after-2026-10-04-windows-gnu.json)

Native tests cover the feature sections and a focused emitter test checks that
an integer-only program omits unused sections. The full workspace and Python
test suites remain the regression gate.
