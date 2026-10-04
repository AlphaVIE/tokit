# Pointer-chase benchmark

This snapshot measures repeated, data-dependent array reads. Every program
accepts an iteration count followed by the integers in `contract.json` as
command-line arguments. Starting at index zero, each iteration reads
`next[index]`, sets `index` to that value, and adds the new index to an `i32`
sum. The printed result is `Ok(sum)`.

The 64 input indices form one cycle (`next[i] = (i + 13) % 64`). The values are
passed at runtime, so the source does not hard-code the route. The runner
validates the full cycle, computes the exact expected sum in O(64) time, and
checks every warmup and measured output. The workload at 40,000,000 iterations
returns `Ok(1260000000)`.

Run the complete matrix with the required toolchains installed:

```sh
python3 scripts/benchmark_matrix.py benchmarks_version_04102026_213748 \
  --iterations 40000000 --warmups 1 --samples 5 --output pointer-chase.json
```

The matrix includes C++, C#, Go, JavaScript, Python, Rust, Tokit, and
TypeScript. Runtime is whole-process elapsed time, including startup; it is
not an isolated array-access latency or throughput measure. Compilation has
one observation per source. Source tokens use `cl100k_base` when `tiktoken`
is installed and represent complete programs, including argument parsing.
The older three-element array-cycle snapshot remains available as historical
context; its Tokit and Rust timings were almost identical to the scalar loop.
