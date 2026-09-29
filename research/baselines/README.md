# Executable baseline protocol

The first executable fixture is [sum-positive](sum-positive/contract.json). Its eight implementations cover Tokit, Rust, Go, C, C++, Python, TypeScript, and Java. `scripts/verify_sum_positive.py` compiles or runs each available implementation and checks every case against the same contract. `--require-all` makes missing toolchains a failure; the CI baseline job uses it. A local run reports tested and skipped languages explicitly.

For Tokit, the harness appends a generated `main` to the marked function source for each input, because this early subset has no command-line argument or standard I/O API. Other baselines parse command-line integers. The `BENCH_START`/`BENCH_END` region isolates the core function for syntax-token comparison; the wrappers are execution adapters and must be included in future full-program workload comparisons.

The contract avoids integer overflow. Python and TypeScript have different general numeric semantics, while C and C++ require care around signed overflow; passing these bounded cases establishes behavior only for the specified inputs. It does not prove full `i32` semantic equivalence. [The measured core-function counts](sum-positive/RESULTS.md) are one small source-only observation, not a speed or overall token advantage. The remaining research fixtures still need executable contracts and baselines.
