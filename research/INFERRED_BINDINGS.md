# Local binding inference experiment

The previous subset required `:Type` on every `let` and `var`. The checker
already computes an initializer type, so this experiment permits an omitted
annotation when that type is complete. Public function signatures, record
fields, and enum payloads remain explicit. Empty arrays and values with an
unknown `Option` or `Result` component still require an annotation (`E115`).
The native backend uses the checked initializer type when emitting its Rust
binding, so interpreter and native semantics agree.

The [new versioned cycle-sum snapshot](../benchmarks_version_04102026_003229/README.md)
removes five local annotations from the Tokit implementation without
changing the contract. With `tiktoken==0.14.0`, its `cl100k_base` source
count falls from 116 to 105 tokens (9.5%); UTF-8 bytes fall from 312 to
287 (8.0%). Python remains shorter at 89 source tokens in this fixture.

The [paired local report](results/inferred-bindings-2026-10-04-windows-gnu.json)
verifies output and retains five timed samples for each available language
and version. Native process medians differ by less than one millisecond;
these sequential observations do not establish a runtime change. The
language change affects source typing and code generation, not the
cycle-sum loop. C++, Go, TypeScript, and .NET SDK 8 were unavailable on this
host, and the report marks those rows incomplete.

This result supports optional local inference as a provisional token-saving
feature. It does not settle broader inference rules or show that Tokit has
the fewest tokens across language families and workloads.
