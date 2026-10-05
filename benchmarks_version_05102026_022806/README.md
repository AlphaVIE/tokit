# Wide-integer array sum benchmark

All eight programs take an iteration count and at least two `i64` values as
runtime arguments. They cycle through the values and accumulate one value per
iteration. Output is `Ok(total)`. Both contracts contain values above the
signed `i32` range, and the measured result exceeds that range too.

`contract.json` supplies 64 values for the primary case. `contract_alt.json`
supplies four different values to check that the programs use runtime input
rather than a fixed total. The matrix runner validates the contract, computes
the expected result independently, rejects totals above signed `i64`, and
checks every warmup and timed output. All values are positive, so the final
bound also bounds every intermediate sum.

Run the primary case with all toolchains installed:

```sh
python3 scripts/benchmark_matrix.py benchmarks_version_05102026_022806 \
  --iterations 40000000 --warmups 1 --samples 5 --output wide-sum.json
```

Pass `--contract benchmarks_version_05102026_022806/contract_alt.json` for the
alternate case. Runtime is whole-process elapsed time, including argument
parsing and startup. This workload combines array reads, index wrapping, and
integer addition. JavaScript and TypeScript use `BigInt` because the measured
sum exceeds their exact `Number` range. Source tokens cover complete programs,
including argument parsing.
