# Type system

Tokit is statically and nominally typed. Every program is fully checked
before it runs; the interpreter and the native backend execute only checked
programs, so a type error can never surface at run time. This document
summarizes the rules implemented in [checker.rs](../compiler/src/checker.rs);
per-feature details are in [EXPERIMENTAL_SUBSET.md](EXPERIMENTAL_SUBSET.md).

## Types

| Type | Written | Values |
| --- | --- | --- |
| 32-bit integer | `i32`, `I` | two's complement; overflow is `E201` |
| 64-bit integer | `i64`, `L` | literals carry `i64`, e.g. `5i64` |
| float | `f64`, `F` | IEEE 754 binary64 |
| boolean | `bool` | `true`, `false` |
| text | `String` | UTF-8, immutable value |
| bytes | `Bytes` | packed bytes 0–255 |
| unit | `Unit` | `()` |
| array | `[T]` | homogeneous, growable with `push` |
| map | `Map<K,V>` | ordered; `K` is `i32`, `i64`, `String`, or `bool` |
| optional | `Option<T>` | `Some(v)`, `None` |
| result | `Result<T,E>` | `Ok(v)`, `Err(e)` |
| task | `Task<T>` | handle from `spawn` |
| function value | `(A,B)->R` | lambdas and named functions |
| record | `struct P<T>{...}` | named fields, built positionally `P(1,2)` |
| enum | `enum E<T>{A,B(T)}` | variants with at most one payload |
| opaque handles | `Conn`, `Listener` | network sockets |

Built-in records `Request`, `Response` and built-in enums `IoError`,
`ParseError`, `TaskError` are reserved names.

## Rules

- **No implicit conversions.** Numeric types never mix; conversions are
  functions named after their target (`i64(x)`, `f64(x)`, `i32(f)` returning
  `Option`). `+` concatenates two `String`s or two arrays of one element type.
- **Equality** (`==`, `!=`, `contains`) is defined for `i32`, `i64`, `f64`,
  `bool`, `String`, and `Bytes`. Arrays, records, enums, options, results,
  maps, functions, tasks, and handles have no `==` (`E104`); compare them with
  `match` or a field-wise function. Ordering operators (`<`, `<=`, `>`, `>=`)
  work on numbers and `String`; `sort` and map keys additionally order
  `bool` (`false` first).
- **Bindings** infer their type from a complete initializer. `[]`, `None`,
  `Ok(x)` without an `Err` type, and `Map()` need an annotation or an expected
  type from context (`E115`). `let` bindings are immutable, `var` bindings may
  be reassigned (`E109`).
- **Functions** declare every parameter and the return type. They may be
  recursive and mutually recursive and may appear in any order.
- **Blocks** have the type of their final expression, or `Unit`. `return`,
  `break`, `continue`, `exit`, and failing `?` have the bottom type, which
  joins with any other type.
- **Control flow types**: both `if` branches and all `match` arms must join to
  one type (`E102`); a missing `else` requires a `Unit` branch.
- **`?`** unwraps `Result` in a function returning a `Result` with the same
  error type, or `Option` in a function returning `Option` (`E111`).
- **Lambdas** take parameter types from the expected function type, else
  from annotations (`E115`). They capture copies of the locals they read and
  cannot `return`, `?`, `break`, or `continue` out of their body.
- **Pattern matching** is checked for exhaustiveness and reachability over
  nested patterns (`E116`).

## Generics

Records, enums, and functions take type parameters: `struct Pair<A,B>{...}`,
`enum Tree<T>{...}`, `first<T>(xs:[T])->Option<T>`. Type arguments are
inferred from arguments and expected types; there is no explicit
instantiation syntax. Generics are parametric: a type parameter supports
only operations valid for every type (passing, storing, returning, matching
on structure). Every declared parameter must be used (`E115`). The native
backend monomorphizes through Rust generics.

## Traits and interfaces

Tokit has no traits, interfaces, or operator overloading, by decision for
this version:

- Operations that need a capability of a type (`==`, ordering, rendering,
  map keys) are defined for a fixed, documented set of types, so a program's
  meaning never depends on which instance is in scope.
- Behavior that varies by type is passed explicitly as a function value —
  `sort_by(xs,|p|p.age)`, `fold(xs,0,|a,x|a+x)`, or a record of functions —
  which costs a few tokens at the call site and nothing at run time after
  monomorphization.
- Without instance resolution, generic code type-checks in one pass and
  error messages never mention unsolved constraints.

Bounded generics (`<T:Ord>`) remain a candidate if measured programs show
that explicit function arguments cost more tokens or repair turns than a
constraint syntax would; such an addition would be backward compatible.

## Soundness boundary

There is no `unsafe`, no null, no uninitialized memory, no casts between
unrelated types, and no shared mutable state: every value is either copied
or immutable when shared. Runtime failures that typing cannot rule out —
integer overflow, division by zero, out-of-range indexes and slices, byte
values outside 0–255, call depth — stop the program with a diagnostic
(`E201`–`E207`) instead of producing undefined behavior.
