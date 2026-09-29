# Task-square executable baseline

The [contract](contract.json) fixes seven bounded `i32` inputs. Each implementation starts a separate task, computes `x*x`, waits for completion, and prints `Ok(square)`. No case overflows or deliberately fails. The copied integer input has no shared state or side effects, so scheduling does not affect the expected output. The `Err(TaskError::Failed)` representation is specified, but failure induction and exact cross-language failure behavior remain unverified. Go's baseline, in particular, does not recover a panic in its goroutine; Python threads also do not promise parallel CPU execution under the usual interpreter.

Tokit's executable candidate A uses `spawn square(x)` and `join(job)?`. The original A/B/C notation probes use `task.spawn` with a capture closure; this prototype has no closures or imports, so the executable A adaptation passes `x` explicitly to a named function. The B and C samples remain notation probes, not executable programs. Closure capture, task cancellation, panic policy, and overflow behavior need owner review before a grammar or runtime contract is frozen.

On the Windows development machine, Tokit, Rust, Python, TypeScript, and Java passed all seven cases. Go, C, and C++ were unavailable locally. CI uses `--require-all` to verify all eight implementations on Linux.

The source between `BENCH_START` and `BENCH_END` contains the core task function. Measured on 2026-09-29 with `tiktoken==0.14.0`, `tokenizers==0.23.2`, and the pinned Qwen tokenizer documented in [the cross-family report](../../CROSS_FAMILY_RESULTS.md):

| Language | Core bytes | cl100k_base | o200k_base | Qwen2.5-Coder |
| --- | ---: | ---: | ---: | ---: |
| Tokit | 124 | 43 | 43 | 48 |
| Rust | 135 | 43 | 43 | 45 |
| Go | 140 | 44 | 44 | 47 |
| C | 420 | 131 | 131 | 133 |
| C++ | 115 | 36 | 36 | 38 |
| Python | 136 | 34 | 35 | 34 |
| TypeScript | 463 | 108 | 109 | 108 |
| Java | 222 | 43 | 44 | 43 |

Reproduce with:

```text
python scripts/verify_task_square.py --require-all
python scripts/measure_task_square.py --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json
```

These are source-only core counts. Tokit ties Rust under two encodings and uses more tokens than C++ and Python in this fixture. The counts omit imports and command-line adapters, and do not measure runtime speed, scheduling, model generation, or repair success.
