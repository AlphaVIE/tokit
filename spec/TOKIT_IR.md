# Experimental typed IR

The native bootstrap now lowers eligible checked functions into a small,
backend-independent scalar IR before emitting Rust. Each instruction defines
one typed value with a source span. Values have sequential IDs, and operands
refer only to earlier instructions. A verifier checks these ordering and type
invariants before backend use.

The current slice supports straight-line scalar functions over `i32`, `i64`,
and `bool`: parameters, literals, checked negation and arithmetic, equality,
integer comparisons, immutable local bindings, scalar expression statements,
and nested lexical blocks. A local name points to the SSA value of its
initializer; nested scopes may shadow it without mutating that value.
Instructions for discarded expressions still execute, preserving checked
arithmetic failures. Checked arithmetic retains its source location for
runtime diagnostics. Functions with mutable bindings, control flow, calls,
other types, or generics still use the checked AST emitter. This fallback
lets the IR grow without changing the meaning of existing programs.

The IR has no control-flow blocks, effects, ownership representation, or
optimization passes yet. It is an architectural starting point, not a stable
serialized format or backend ABI. Next work should add explicit control-flow
blocks and branches, then calls; each expansion needs interpreter and native
parity tests before moving another construct off the AST path.
