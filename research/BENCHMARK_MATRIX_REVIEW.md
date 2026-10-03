# First review of the versioned benchmark matrix

The [cycle-sum snapshot](../benchmarks_version_02102026_014800/README.md)
is a useful small contract fixture, but the initial runner reported one
runtime observation without checking the program's result. It also displayed
a heuristic count as model tokens when `tiktoken` was unavailable, treated
source-reading time as a language initialization metric, and could download
TypeScript through `npx` during a run. Those measurements could support
incorrect comparisons.

The revised runner checks exact output on every warmup and timed run, keeps
all five timed samples in the raw report, and displays their median. It
records source hashes, tokenizer and tool versions, and distinguishes
unavailable tools from failed builds or wrong answers. Token counts are
unavailable without the real tokenizer. The runner compiles each language
once and times whole process execution, including startup.

The [Windows GNU report](results/benchmark-matrix-2026-10-04-windows-gnu.json)
is deliberately marked incomplete: this host lacks a C++ compiler, Go,
TypeScript, and .NET SDK 8. At 300,000 iterations, Tokit-native and Rust
had process-runtime medians of about 7.1 ms each; Python was about 32 ms
and JavaScript about 42 ms. These samples do not isolate loop throughput,
and the single build observations are not enough for build-time claims.
The `cl100k_base` tokenizer counted 116 Tokit source tokens and 89 Python
source tokens. This directly contradicts the old snapshot's expectation that
Tokit would necessarily be shortest.

The next snapshot should add an input with allocation and data access, run
all languages on a fully provisioned host, and include at least two input
sizes to separate launch cost from workload cost. It should retain the
same contract checks and raw-sample reporting. Token counts remain source
measurements; generation and repair token budgets need a separate study.
