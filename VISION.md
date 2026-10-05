# Tokit vision

Tokit tests whether a programming language designed as a compact, deterministic exchange format between a code-generating agent and a compiler can reduce the **total cost of building correct software**. A human states intent and inspects semantic explanations; an agent emits source or structured edits; the compiler checks and translates them to executable code.

This is a research hypothesis, not a claim that dense syntax is better. Source token count, model generation success, repair cost, compile time, runtime performance, and safety must be measured together. A shorter spelling that increases mistakes may lose overall.

## Principles

1. The compiler is the authority on program meaning. Implicit conversions, invisible normal control flow, and ambiguous parsing are unacceptable.
2. One canonical persistent representation should be preferred until evidence supports a second one.
3. Human auditability comes from `tok explain`, diagnostics, source maps, and documentation, as well as source text.
4. Native performance, memory safety by default, and reproducible builds are targets to verify, not properties to announce in advance.
5. The language core remains small. Libraries and tools provide broad applicability.
6. Decisions remain reversible before a versioned specification is approved.

## Current status

The repository now has an experimental Rust compiler, reference interpreter,
and Rust-based native bootstrap for a checked `.tok` subset. Its syntax and
runtime contracts remain provisional. The candidate notation under
`research/candidates/` is separate from accepted `.tok` examples. See the
[phase-by-phase implementation status](IMPLEMENTATION_STATUS.md) for current
coverage and missing work.

## Decision boundary

Research can narrow candidates, but public syntax, type and error semantics, memory guarantees, and backend commitments require an explicit design review before being frozen. See [LANGUAGE_CANDIDATES.md](LANGUAGE_CANDIDATES.md) and [RESEARCH_PLAN.md](RESEARCH_PLAN.md).
