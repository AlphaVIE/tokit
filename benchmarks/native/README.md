# Native bootstrap benchmark

`cycle_sum.tok` and `cycle_sum.rs` repeatedly add the cycle `1, 2, 3` for a
command-line supplied number of iterations. Both use checked `i32` addition,
parse one argument, and print `Ok(<sum>)`. The harness accepts only positive
multiples of three up to 100 million iterations, so the expected sum is exactly
twice the iteration count and does not overflow. It checks both outputs before
timing and after every timed run.

Run from the repository root:

```sh
python3 scripts/bench_native.py --output native-results.json
```

On Windows with the GNU Rust toolchain, pass
`--rust-toolchain stable-x86_64-pc-windows-gnu`. Use `--iterations`,
`--warmups`, `--samples`, and `--build-samples` to change the protocol.
The CI job uses a small smoke input to check that the harness works; its
numbers are not performance results.

The script first prepares the debug `tok` CLI with Cargo; this preparation is
recorded separately and excluded from build timing. It then times three
independent `tok build` and `rustc` invocations by default. Both native outputs
are optimized with `rustc -C opt-level=2` through the same installed toolchain.
The Tokit build includes checking and Rust source generation as well as its
`rustc` invocation. Binary sizes are the final executable file sizes, including
platform runtime and linker effects.

The script validates both binaries, runs two untimed warmups, then times seven
process launches for each binary, alternating order between rounds. Runtime
includes process startup, argument parsing, computation, and output. JSON
contains all raw nanosecond samples, medians, minimums, maximums, source
hashes, host details, and tool versions. Results should be compared only on
the same host and toolchain. This one arithmetic workload cannot establish
overall language performance, allocation behavior, or native-backend quality.

## Initial local result

The [raw Windows GNU result](windows-gnu-initial.json) records one run on
September 29, 2026, using Rust 1.98.1. With 99,999,999 iterations, median
build times were 472 ms for Tokit and 291 ms for Rust. Median process runtimes
were 50.7 ms and 50.9 ms, respectively; their sample ranges overlap. Native
binaries were 4,978,899 and 4,962,890 bytes. These observations motivate
tracking bootstrap build overhead. The runtime samples do not support a claim
that either implementation is faster on this workload.
