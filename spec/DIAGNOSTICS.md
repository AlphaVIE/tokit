# Diagnostics

Every compiler and runtime message has a stable code. `tok check --json`
returns `{"ok":false,"error":{"code","line","column","message",...}}`; text
output is `CODE@line:column message`. Codes are grouped by stage:
`E0xx` lexing and parsing, `E1xx` checking and loading, `E2xx` running,
`E3xx` native builds, `Wxxx` advisory lint warnings from `tok lint`. A test keeps this list equal
to the codes the compiler emits.

| Code | Stage | Meaning | Typical fix |
| --- | --- | --- | --- |
| `E001` | lex | invalid character | remove it or put it in a string |
| `E002` | parse | syntax error: expected token, expression, pattern, identifier; misplaced import; `break` outside a loop; invalid assignment target | follow the message's expected token |
| `E003` | lex/parse | numeric literal out of range or malformed (`i32`, `i64`, `f64`, exponent) | use a suffix (`5i64`) or a smaller value |
| `E004` | lex | unclosed string or unknown escape | close the string; escapes are `\n \t \r \" \\` |
| `E101` | check | unknown name or function, or calling a non-function value | declare it, fix the spelling, or import it |
| `E102` | check | type mismatch: argument, binding, return, branch, arm, or array element | the message shows expected and actual types |
| `E103` | check | unknown or wrongly applied type, invalid map key type, unknown enum | fix the type name or its arguments |
| `E104` | check | invalid operands for an operator or builtin (e.g. `==` on records, mixing `i32` and `i64`) | convert explicitly or compare with `match` |
| `E105` | check | wrong number of arguments | match the declared parameter list |
| `E106` | check | duplicate or reserved name: binding, field, function, parameter, type, variant | rename |
| `E107` | check | unreachable code after `return`, `break`, or `continue` | remove it |
| `E109` | check | assigning to or pushing onto an immutable binding | declare it with `var` |
| `E110` | check | invalid indexing, iteration, push, or map entry access | use `get(m,k)` for maps; index arrays and `Bytes` only |
| `E111` | check | invalid `?`: wrong operand, wrong return type, or inside a lambda | return a matching `Result`/`Option`, or use `match` |
| `E112` | check | directly recursive record or enum layout | recurse through an array |
| `E113` | check | invalid field access; method-call syntax gets a hint (`len(x)`, `String(x)`, `xs.push(v);`) | use the suggested function or statement |
| `E114` | check | unknown or unqualified enum variant, wrong payload | write `Enum::Variant(payload)` |
| `E115` | check | a type cannot be inferred, or a type parameter is unused | add an annotation such as `let xs:[I]=[];` |
| `E116` | check | invalid match: unsupported scrutinee, inapplicable, duplicate, or unreachable pattern, or a missing case (named in the message) | add the missing arm or remove the dead one |
| `E117` | check | `spawn` of a call that is not a named, non-generic, pure function | move effects out of the spawned function |
| `E118` | load | invalid or unresolvable import, duplicate alias | fix the import path or alias |
| `E119` | check | using a private declaration of another module | mark it `pub` |
| `E120` | load | package import rules: escaping the package, unpinned source, unknown dependency | use `pkg:name` and declare the dependency |
| `E121` | load | `tok.lock` missing or stale | run `tok lock <entry.tok>` |
| `E201` | run | integer overflow, division by zero, or shift out of range | check bounds or use `i64` |
| `E202` | run | call depth over 10,000 | use a loop |
| `E203` | run | `main` is missing, or `main`, a test, or a benchmark has the wrong signature | remove parameters; tests return `bool` or `Result<bool,E>` |
| `E204` | run | internal evaluator failure (should not happen for checked programs) | report it |
| `E205` | run | index or slice out of range | check `len` first |
| `E206` | run | array length exceeds `i32` | split the data |
| `E207` | run | byte value outside 0–255 | mask with `bit_and(x,255)` |
| `E301` | build | cannot create the temporary generated Rust source | check the temp directory and disk space |
| `E302` | build | `rustc` could not be started or rejected the generated Rust | install Rust or set `TOKIT_RUSTC`; otherwise report it as a backend bug |
| `W001` | lint | a binding is never read | remove it or prefix `_` |
| `W002` | lint | a `var` is never changed | use `let` |
| `W003` | lint | a private function is never called | remove it or make it `pub` |

Codes are never reused for a different meaning; `E108` is unassigned. A change
of meaning gets a new code; message wording may improve between versions.
