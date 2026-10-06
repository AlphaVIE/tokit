# Experimental typed IR

The native bootstrap now lowers eligible checked functions into a small,
backend-independent scalar IR before emitting Rust. Each instruction defines
one typed value with a source span. Values have sequential IDs, and operands
refer only to earlier instructions. A verifier checks these ordering and type
invariants before backend use.

The current slice supports scalar functions over `i32`, `i64`, `f64`, and `bool`:
parameters, literals, checked integer negation and arithmetic, IEEE 754 float
arithmetic and comparisons, equality, immutable local bindings, scalar expression statements, nested
lexical blocks, `if` expressions, short-circuit boolean operators, calls
to non-generic scalar functions, and the infallible conversions `i64(i32)`,
`f64(i32)`, and `f64(i64)` as a typed `Convert` instruction.
A local name points to the SSA value of its initializer; nested scopes may
shadow it without mutating that value.
An `if` instruction owns two nested regions. The verifier checks each region
against the values available before the branch; values created in one branch
cannot be used by the other or by later outer instructions. The native emitter
evaluates only the selected region. `&&` and `||` use the same lazy regions;
the verifier accepts the left operand as the skipped branch's result. Call instructions refer to checked scalar
signatures and evaluate argument instructions in source order. Each emitted
function retains the same call-depth guard as the AST backend.
Instructions for discarded expressions still execute, preserving checked
arithmetic failures. Checked arithmetic retains its source location for
runtime diagnostics. Functions with mutable bindings, loops, non-scalar types,
or generics still use the checked AST emitter. This fallback
lets the IR grow without changing the meaning of existing programs.

The IR has nested conditional regions but no general control-flow graph,
loops, effects, ownership representation, or optimization passes yet. It is an
architectural starting point, not a stable serialized format or backend ABI.
Next work should add loop-capable control-flow blocks and effect metadata;
each expansion needs interpreter and native parity tests before moving another
construct off the AST path.
