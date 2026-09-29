# Additional syntax comparison fixtures

These are small language-neutral contracts. The matching `a`, `b`, and `c` files under each directory must carry the same operations. They are probes of representation cost, not executable Tokit programs.

| Fixture | Contract |
| --- | --- |
| `sum-positive` | Sum only positive signed 32-bit integers in an array, starting from zero. Overflow is an error in a future executable version; current notation does not yet encode that policy. |
| `checked-division` | Divide `a` by `b` as signed 32-bit integers. Return `Err(DivZero)` if `b` is zero, otherwise `Ok(a / b)`. Signed overflow behavior remains to be specified. |
| `generic-pair` | Define a generic two-field `Pair<T>` and return a fresh pair with its fields reversed. The values themselves are not transformed. |
| `line-count` | Read a UTF-8 file, propagating an I/O error, and count elements emitted by `text.lines()`. Line splitting and integer overflow need exact semantics before execution. |
| `task-square` | Spawn a task that computes `x*x`, join it, propagate a task error, and return `Ok(square)`. Capture/lifetime and overflow semantics remain open. |

These deliberately expose missing semantics. An implementation must settle them before any candidate can claim verified behavioral equivalence. Do not use these files as benchmark evidence for runtime or safety.
