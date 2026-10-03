# Versioned benchmark snapshot

This folder captures one comparable cycle-sum workload in Tokit, JavaScript, TypeScript, Go, C++, C#, Python, and Rust.

All implementations follow the same contract:

- read exactly one command-line argument
- parse it as an integer iteration count
- add the repeating cycle `1, 2, 3`
- print `Ok(<sum>)`

Source bytes and model tokens are separate measurements. The shortest file may
differ by metric and tokenizer. This fixture includes complete command-line
handling and output code in every language; any size claim must use the
measured files and an identified tokenizer.

Build time and process runtime are separate measurements. Process runtime
includes launch and initialization; the single cycle-sum input cannot isolate
loop throughput. Use multiple input sizes and repeated runs before attributing
a difference to steady-state execution.

This snapshot is meant as a comparative fixture, not as a general performance claim.

Run `python scripts/benchmark_matrix.py --iterations 300000 --warmups 1 --samples 5 --output result.json`
from the repository root. The harness checks every exit status and exact
`Ok(<sum>)` output, records each timed runtime sample, and reports their
median. The raw report includes source SHA-256 digests. It times one build for compiled languages. Runtime includes process
startup. Missing tools are reported explicitly and mark the report incomplete.
Token counts use
`cl100k_base` through `tiktoken`; the JSON output records its installed
version, or leaves counts unavailable when it is absent. For a reproducible
token comparison, install `tiktoken==0.14.0`. The accepted iteration range is
`0..1073741823`, keeping the sum within Tokit's `i32` result type. Runtime
comparisons need repeated runs on the same host with recorded tool versions;
this harness is exploratory rather than a cross-language ranking.

Future snapshots can increase complexity in small steps instead of changing the workload all at once. A sensible progression is to keep this cycle-sum core, then add one extra dimension per version, such as array access, checked error handling, file I/O, or a second function call path. That keeps the source comparison stable while making runtime and translation cost more representative over time.
