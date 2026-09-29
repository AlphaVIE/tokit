# Tokit

Tokit explores a machine-first programming language optimized for the total cost of producing, checking, and repairing programs with language models. The language design is experimental; no source syntax has been selected yet.

The project brief is in [PROMPT.txt](PROMPT.txt). Start with [VISION.md](VISION.md), [DESIGN_GOALS.md](DESIGN_GOALS.md), and [RESEARCH_PLAN.md](RESEARCH_PLAN.md). The [candidate comparison](LANGUAGE_CANDIDATES.md) and [initial measurements](research/INITIAL_RESULTS.md) are experiments, not a frozen syntax.

An [experimental candidate A compiler slice](spec/EXPERIMENTAL_SUBSET.md) can parse, type-check, interpret, explain, [measure structural density](research/STRUCTURAL_METRICS.md), and [build native executables](spec/NATIVE_BOOTSTRAP.md) from a small `.tok` subset. With Rust installed, run `cargo test --workspace`, `cargo run -p tokit-compiler --bin tok -- run examples/answer.tok`, or `cargo run -p tokit-compiler --bin tok -- explain examples/match_result.tok`. The source grammar and bootstrap backend are not yet production language commitments.
