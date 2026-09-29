# Experimental candidate A subset (not a stable Tokit specification)

This document describes exactly what the current Rust prototype accepts. It is a test vehicle for syntax and compiler architecture. The canonical language grammar has **not** been selected.

## Accepted constructs

```text
program  = (function | record | enum)+ ;
function = "fn" identifier generic-params? "(" parameters? ")" "->" type block ;
record   = "struct" identifier generic-params? "{" (identifier ":" type ("," identifier ":" type)*)? "}" ;
enum     = "enum" identifier "{" (enum-variant ("," enum-variant)*)? "}" ;
enum-variant = identifier ("(" type ")")? ;
generic-params = "<" identifier ("," identifier)* ">" ;
parameters = identifier ":" type ("," identifier ":" type)* ;
type     = "i32" | "bool" | "String" | "Unit" | "[" type "]"
         | "Result" "<" type "," type ">" | "Option" "<" type ">"
         | "Task" "<" type ">" | identifier
         | identifier "<" type ("," type)* ">" ;
block    = "{" statement* expression? "}" ;
statement = ("let" | "var") identifier ":" type "=" expression ";"
          | identifier "=" expression ";"
          | identifier "." "push" "(" expression ")" ";"
          | "for" identifier "in" expression block
          | "while" expression block
          | "return" expression ";"
          | expression ";" ;
expression = integer | string | "true" | "false" | identifier | "[" arguments? "]"
           | identifier "::" identifier ("(" expression ")")?
           | "Ok" "(" expression ")" | "Err" "(" expression ")"
           | "Some" "(" expression ")" | "None"
           | identifier "(" arguments? ")"
           | "spawn" identifier "(" arguments? ")"
           | "(" expression ")" | block
           | "if" expression block "else" (block | expression-if)
           | "match" expression "{" (pattern "=>" expression ("," pattern "=>" expression)* ","?)? "}"
           | expression "?" | expression "[" expression "]" | expression "." identifier
           | expression binary-op expression ;
pattern = "Ok" "(" identifier ")" | "Err" "(" identifier ")"
        | "Some" "(" identifier ")" | "None"
        | identifier "::" identifier ("(" identifier ")")? | "true" | "false" ;
binary-op = "+" | "-" | "*" | "/" | "==" | "!=" | "<" | "<=" | ">" | ">=" ;
```

Binary operators use normal arithmetic precedence, with equality below comparisons. Blocks return their final expression; a block without a final expression has type `Unit`. `return` exits the current function, including from a loop. `if` requires `else`, and both branches must have compatible types. Functions may call later functions and recurse. Local bindings require explicit types. `let` is immutable and `var` permits reassignment; shadowing an outer binding in a nested block is currently permitted, while duplicate names in one block are rejected. Arrays are homogeneous and can be empty when an expected array type provides context. `for` iterates over an array value snapshot; its loop variable is scoped to one iteration. Arrays are copied as values, so changing one binding does not mutate copies. Postfix `array[index]` reads an element; the index is `i32`, and negative or out-of-range indices report `E205`. `xs.push(value);` appends a typed element to a mutable `var` array binding, evaluating the value before mutation. A `let` binding reports `E109`, a non-array reports `E110`, and a wrong element type reports `E102`. This is currently a statement on a direct local name; indexed mutation and record-field mutation are not supported.

`while condition { ... }` re-evaluates a `bool` condition before each iteration. A false initial condition skips the body, body-local bindings are scoped to each iteration, and mutation of outer `var` bindings remains visible to later conditions. `return` and `?` may exit the surrounding function from the condition or body. The checker validates the body even if a condition is literally false. There is no `break` or `continue` yet; loops can still run indefinitely if their condition never becomes false.

`Result<T,E>` is a typed success/error value. `Ok(value)` and `Err(value)` produce the corresponding variant; the missing side of each constructor is inferred from the expected type or the other branch. Postfix `?` unwraps `Ok` and immediately returns `Err` from the current function. The enclosing function must return a `Result` with a compatible error type. Normal errors are values, while checked arithmetic failures still use the prototype's runtime diagnostic `E201`; unifying these models remains an open semantic decision.

`Option<T>` represents a present value with `Some(value)` or an absent value with `None`; there is no unrestricted null value. `None` gets its element type from a declared return, binding, field, or another expression. Matching an untyped bare `None` reports `E115`. A match on `Option<T>` must cover both `Some(binding)` and `None`. `Option<T>` has an inline value layout, so directly recursive records or enums through `Option` report `E112`; arrays and tasks provide indirection. `?` currently applies only to `Result`, not `Option`.

`i32` arithmetic is checked in the interpreter: overflow and division by zero produce `E201`. Integer literals must fit `i32`. This subset has no unary minus, so negative literals cannot currently be written directly; subtraction can produce negative values. These are prototype limitations, not final numeric semantics.

Strings are UTF-8 values written in double quotes. Literals accept direct Unicode and the escapes `\n`, `\r`, `\t`, `\"`, and `\\`; a raw line break or unknown escape is `E004`. `+` concatenates two strings, and `==` / `!=` compare their contents. Values print with quotes and escaped control characters, including inside arrays and results. This syntax is experimental.

Named records declare typed fields in order. Calling the record name constructs a value with one argument per field; postfix `.field` reads a field. Records are value types, copied when passed or read. Unknown type names report `E103`, duplicate names or fields `E106`, direct recursive value layouts `E112`, and invalid field access `E113`. Recursion through an array is permitted because the array's storage is indirect.

Records and functions can declare type parameters, as in `struct Pair<T>{left:T,right:T}` and `fn flip<T>(p:Pair<T>)->Pair<T>{Pair(p.right,p.left)}`. A call infers each type argument from its value arguments; there is no explicit call-site specialization yet. Ambiguous calls or unused record type parameters report `E115`. Type parameters have implicit clone and render capabilities in the native bootstrap; trait bounds and general constraint solving are not implemented. A generic `main` is invalid.

Enums declare named variants, including an optional empty variant set. A variant may carry one typed value, for example `enum Event{Stop,Number(i32)}`. `Event::Stop` and `Event::Number(7)` construct values, which may also serve as typed `Result` errors. Missing, extra, or incorrectly typed payloads are rejected. Unknown variants or using an enum name as a function report `E114`. Direct recursive value layouts across records and enums report `E112`; recursion through an array is permitted because its storage is indirect. Generic enums and variants with multiple payload fields are not yet supported.

`match` evaluates its scrutinee once and chooses an arm by pattern. It supports `Result` with `Ok(name)` and `Err(name)` payload bindings, `Option` with `Some(name)` and `None`, enums with qualified `Enum::Variant` and `Enum::Variant(name)` patterns, and `bool` with `true` and `false` patterns. All possible cases must occur exactly once; missing, duplicate, or inapplicable patterns report `E116`. A payload binding is scoped to its arm and has the declared payload type. Arms must have compatible result types; `return` may exit the enclosing function from an arm. This syntax and exhaustiveness policy are experimental.

The provisional library includes `read_text(String)->Result<String,IoError>` and `lines(String)->[String]`. `IoError` is a reserved enum with `Denied`, `NotFound`, `InvalidUtf8`, and `Other` variants. File reads require an explicit path grant with `tok run --allow-read <path> file.tok` or the same flag on a generated binary. See the [filesystem capability contract](FILESYSTEM_CAPABILITY.md); this is not an OS sandbox.

`len<T>([T])->i32` returns an array length and reports `E206` if it exceeds `i32` (a theoretical limit for this prototype). An untyped empty literal cannot infer `T`; give it a declared array type first. `parse_i32(String)->Result<i32,ParseError>` accepts an optional ASCII `+` or `-` followed by decimal digits; whitespace, empty strings, and other characters yield `ParseError::Invalid`, while a syntactically valid number outside the `i32` range yields `ParseError::OutOfRange`. `ParseError` is a reserved enum with those two variants. Both functions are pure and can be used inside a spawned pure function. See [parse_argument.tok](../examples/parse_argument.tok) for a CLI input example.

[parse_numbers.tok](../examples/parse_numbers.tok) builds an array from arbitrarily many CLI arguments with `push`, returning a typed parse error on the first invalid input.

`args()->[String]` returns the Tokit program's UTF-8 command-line arguments. For the interpreter, write `tok run [--json] [--allow-read <path>] file.tok -- argument...`; the file path, separator, and launcher flags are omitted from `args()`. A native binary accepts `./program [--allow-read <path>] -- argument...` and likewise omits its own name, the optional read grant, and the separator. A native program also accepts plain positional arguments without `--` when the first argument is not the reserved `--allow-read` launcher flag. `args()` is an environment effect and cannot be called by a spawned pure function. Non-UTF-8 OS arguments are outside this subset.

`spawn f(args)` creates a `Task<T>` for a named, non-generic user function returning `T`. `join(task)` returns `Result<T,TaskError>`, and `TaskError` currently has the single variant `Failed`. A task value can be copied and joined more than once; each join observes the same result. The native bootstrap runs the function on an OS thread. The reference interpreter evaluates it eagerly so it can check ordinary successful results, but it is not a concurrency simulator. The checker admits only functions whose transitive calls avoid `read_text`, `join`, and nested `spawn`; arguments to `spawn` are evaluated before starting the task. Local mutation and array iteration inside a task are permitted. Generic functions cannot currently be spawned. This is a provisional effect boundary, not the final Tokit concurrency design. An uncaught task panic maps to `TaskError::Failed` in the native bootstrap; existing fatal runtime diagnostics such as `E201` still terminate the process.

## Diagnostics and commands

`tok check file.tok` lexes, parses, and type-checks; `tok run file.tok` additionally evaluates `main()` in the reference interpreter. A run requires a parameterless, non-generic `main`. `tok explain file.tok` prints a deterministic summary of declarations, call relationships, record constructions, and notable operations after type-checking. It currently reports syntactic operations; it does not prove absence of effects in callees. `tok fmt file.tok` prints [canonical experimental whitespace](FORMATTER.md), with `--check` and `--write` modes. `tok stats file.tok` emits JSON structural counts for a checked program; see [structural metrics](../research/STRUCTURAL_METRICS.md). `tok build file.tok -o output` produces a host executable through the experimental Rust bootstrap. `--json` before the path emits a JSON result or diagnostic with byte span, line, and column. Diagnostics use `E001` invalid character, `E002` parse error, `E003` integer literal range, `E004` invalid string literal, `E101` unknown name, `E102` type mismatch, `E103` unknown type, `E104` invalid operands, `E105` arity mismatch, `E106` duplicate name, `E107` unreachable code, `E109` immutable assignment, `E110` invalid array operation, `E111` invalid error propagation, `E112` recursive value layout, `E113` invalid field access, `E114` invalid enum variant, `E115` type inference failure, `E116` invalid match pattern or coverage, `E117` invalid spawn target, `E201` arithmetic failure, `E202` call-depth limit, `E203` invalid entry point, `E204` interpreter invariant failure, `E205` array index out of bounds, and `E206` array length outside `i32`. Error output contains a source line and column. GC, a full standard library, imports, and trait-constrained generics do not yet exist in this prototype.

## Reproduce

On a host with a Rust toolchain and linker:

```text
cargo test --workspace
cargo run -p tokit-compiler --bin tok -- run examples/answer.tok
```

The repository pins Rust 1.98.1. The current Windows development machine uses `cargo +stable-x86_64-pc-windows-gnu` because no MSVC linker is installed. This is a local toolchain choice, not a project ABI decision.
