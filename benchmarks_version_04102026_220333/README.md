# Generated-array chase benchmark

All eight programs take `iterations`, `size`, and `stride` as runtime
arguments. They build an `i32` array with `next[i] = (i + stride) % size`,
then repeatedly set `index = next[index]`. Each step adds one to the result
when the new index is below `size / 2`. Output is `Ok(result)`.

`contract.json` is the primary case: 262,144 elements and stride 8,191.
`contract_small.json` uses 16,384 elements with the same stride. The two
`*_stride13.json` contracts vary the stride while keeping those sizes.
Every size/stride pair is coprime, so the chase visits every element before
returning to index zero. The matrix runner validates that property, computes
the expected count without running the measured program, and checks every
warmup and timed output.

Run the primary case with all toolchains installed:

```sh
python3 scripts/benchmark_matrix.py benchmarks_version_04102026_220333 \
  --iterations 40000000 --warmups 1 --samples 5 --output large.json
```

Pass `--contract benchmarks_version_04102026_220333/contract_small.json`
to measure the small case with the same source files. The other contracts
work the same way. Runtime is whole-process elapsed time, including array
construction and startup. The four cases examine the joint effect of size
and access stride; they do not isolate cache misses or memory bandwidth.
Source tokens cover complete programs, including argument parsing.
