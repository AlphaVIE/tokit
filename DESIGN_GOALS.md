# Design goals and evidence gates

## Priority and measurement

| Goal | Initial measure | Gate before claiming success |
| --- | --- | --- |
| Low model context cost | Source tokens by tokenizer; generated and repair tokens by model | Equivalent workloads, several model families, confidence intervals |
| Correct generation | Parse/type-check pass rate; test pass rate; retries | Held-out tasks and fixed prompts, repeated runs |
| Native execution speed | Wall time, CPU time, peak memory, allocations | Equivalent optimized implementations, repeatable environment |
| Reliability and safety | Static rejection cases, runtime failure cases, fuzzing | Executable specification and adversarial tests |
| Deterministic semantics | Parse ambiguity count, canonical format idempotence, reproducible outputs | Parser and formatter corpus; same-input build checks |
| Compile speed | Cold and incremental compile latency | Stable hardware and pinned toolchains |
| Interoperability | ABI conformance and integration tests | Real C and later ecosystem fixtures |
| Human auditability | Accuracy and usefulness of explanation and diagnostics | Human review tasks against source and execution |

The ordering in the brief gives token cost and native performance highest importance, while correctness and safety are mandatory constraints. A weighted score cannot excuse a safety failure. We will record raw measures before aggregating them.

## Primary cost metric

For a programming task, record `C_total = T_prompt + T_initial + T_feedback + T_repair + T_repeated_context`, counting model input and output separately where pricing or context pressure differs. Also report success probability and expected cost per successful solution: `E[C_success] = sum(cost of all attempts) / number of verified successes`. Runs that never succeed remain in the denominator of success rate and in spent cost; do not silently discard them.

Source tokens alone are a diagnostic metric. Character and byte counts are included to expose encoding effects. Later, AST nodes and semantic operations will normalize density, with a documented counting rule.

## Non-goals for the first milestone

- Claiming universal superiority over existing languages.
- Finalizing syntax based on a toy example or one tokenizer.
- Promising a particular GC, optimizer, package registry, or GPU backend before prototype evidence.
- Treating a candidate notation as an executable language.

## First milestone exit criteria

1. An equivalent, reviewed corpus for three distinct candidate representations and baseline languages.
2. Reproducible token counts across at least two genuinely different tokenizer families.
3. A generation-and-repair trial with executable validators or a clearly identified proxy.
4. A decision record that explains tradeoffs and owner-reviewed commitments.
5. A small Rust compiler slice planned against that record, with tests preceding performance claims.
