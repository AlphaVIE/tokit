# Array-access cycle benchmark

This snapshot extends the [cycle-sum contract](../benchmarks_version_02102026_014800/README.md).
Each implementation reads one valid nonnegative iteration count followed by
three array values. The runner supplies `1, 2, 3` from `contract.json` as
separate runtime arguments. The program indexes that array on every iteration, adds the
selected element, advances the index around the array, and prints
`Ok(<sum>)`. The tested range is `0..1073741823`, keeping the result within
Tokit's `i32`. The output oracle is the same as in the cycle-sum snapshots,
but the array access and length check make the runtime work different.

The values arrive at runtime so the compiler cannot assume a constant array.
The Tokit and Rust sources use checked indexing and checked arithmetic.
Other languages use their normal array/list indexing; the index is always
valid for the accepted input domain. The C++ implementation uses
`std::vector::at` for checked access. Native compilers may optimize the
three-element array, so this fixture does not prove general heap-access performance.
It measures whole-process runtime, including startup, and source tokens for
the complete command-line program.

Compare this folder with the inferred-binding cycle snapshot at the same
iteration count:

```sh
python scripts/benchmark_matrix.py \
  benchmarks_version_04102026_003229 \
  benchmarks_version_04102026_005141 \
  --iterations 10000000 --warmups 1 --samples 5 --output result.json
```

The runner verifies every output and marks unavailable toolchains explicitly.
It does not isolate allocations, CPU time, or loop throughput from process
startup. The two snapshot families should be compared on one host with the
same installed toolchains and a pinned tokenizer.
