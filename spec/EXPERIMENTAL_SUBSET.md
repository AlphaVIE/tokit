# Experimental candidate A subset (not a stable Tokit specification)

This document describes exactly what the current Rust prototype accepts. It is a test vehicle for syntax and compiler architecture. The canonical language grammar has **not** been selected.

## Accepted constructs

```text
program  = (function | record | enum)+ ;
function = "fn" identifier generic-params? "(" parameters? ")" "->" type block ;
record   = "struct" identifier generic-params? "{" (identifier ":" type ("," identifier ":" type)*)? "}" ;
enum     = "enum" identifier "{" (identifier ("," identifier)*)? "}" ;
generic-params = "<" identifier ("," identifier)* ">" ;
parameters = identifier ":" type ("," identifier ":" type)* ;
type     = "i32" | "bool" | "String" | "Unit" | "[" type "]"
         | "Result" "<" type "," type ">" | identifier
         | identifier "<" type ("," type)* ">" ;
block    = "{" statement* expression? "}" ;
statement = ("let" | "var") identifier ":" type "=" expression ";"
          | identifier "=" expression ";"
          | "for" identifier "in" expression block
          | "return" expression ";"
          | expression ";" ;
expression = integer | string | "true" | "false" | identifier | "[" arguments? "]"
           | identifier "::" identifier
           | "Ok" "(" expression ")" | "Err" "(" expression ")"
           | identifier "(" arguments? ")"
           | "(" expression ")" | block
           | "if" expression block "else" (block | expression-if)
           | "match" expression "{" (pattern "=>" expression ("," pattern "=>" expression)* ","?)? "}"
           | expression "?" | expression "[" expression "]" | expression "." identifier
           | expression binary-op expression ;
pattern = "Ok" "(" identifier ")" | "Err" "(" identifier ")"
        | identifier "::" identifier | "true" | "false" ;
binary-op = "+" | "-" | "*" | "/" | "==" | "!=" | "<" | "<=" | ">" | ">=" ;
```

Binary operators use normal arithmetic precedence, with equality below comparisons. Blocks return their final expression; a block without a final expression has type `Unit`. `return` exits the current function, including from a loop. `if` requires `else`, and both branches must have compatible types. Functions may call later functions and recurse. Local bindings require explicit types. `let` is immutable and `var` permits reassignment; shadowing an outer binding in a nested block is currently permitted, while duplicate names in one block are rejected. Arrays are homogeneous and can be empty when an expected array type provides context. `for` iterates over an array value; its loop variable is scoped to one iteration. Arrays are copied as values in this interpreter. Postfix `array[index]` reads an element; the index is `i32`, and negative or out-of-range indices report `E205`. Mutation of array elements and append are not yet supported.

`Result<T,E>` is a typed success/error value. `Ok(value)` and `Err(value)` produce the corresponding variant; the missing side of each constructor is inferred from the expected type or the other branch. Postfix `?` unwraps `Ok` and immediately returns `Err` from the current function. The enclosing function must return a `Result` with a compatible error type. Normal errors are values, while checked arithmetic failures still use the prototype's runtime diagnostic `E201`; unifying these models remains an open semantic decision.

`i32` arithmetic is checked in the interpreter: overflow and division by zero produce `E201`. Integer literals must fit `i32`. This subset has no unary minus, so negative literals cannot currently be written directly; subtraction can produce negative values. These are prototype limitations, not final numeric semantics.

Strings are UTF-8 values written in double quotes. Literals accept direct Unicode and the escapes `\n`, `\r`, `\t`, `\"`, and `\\`; a raw line break or unknown escape is `E004`. `+` concatenates two strings, and `==` / `!=` compare their contents. Values print with quotes and escaped control characters, including inside arrays and results. This syntax is experimental.

Named records declare typed fields in order. Calling the record name constructs a value with one argument per field; postfix `.field` reads a field. Records are value types, copied when passed or read. Unknown type names report `E103`, duplicate names or fields `E106`, direct recursive value layouts `E112`, and invalid field access `E113`. Recursion through an array is permitted because the array's storage is indirect.

Records and functions can declare type parameters, as in `struct Pair<T>{left:T,right:T}` and `fn flip<T>(p:Pair<T>)->Pair<T>{Pair(p.right,p.left)}`. A call infers each type argument from its value arguments; there is no explicit call-site specialization yet. Ambiguous calls or unused record type parameters report `E115`. Type parameters have implicit clone and render capabilities in the native bootstrap; trait bounds and general constraint solving are not implemented. A generic `main` is invalid.

Unit enums declare named variants, including an optional empty variant set. `Enum::Variant` constructs a value and can be used as a typed `Result` error. Unknown variants or using an enum name as a function report `E114`. Payload variants are not yet supported.

`match` evaluates its scrutinee once and chooses an arm by pattern. It supports `Result` with `Ok(name)` and `Err(name)` payload bindings, unit enums with qualified `Enum::Variant` patterns, and `bool` with `true` and `false` patterns. All possible cases must occur exactly once; missing, duplicate, or inapplicable patterns report `E116`. A payload binding is scoped to its arm and has the corresponding result type. Arms must have compatible result types; `return` may exit the enclosing function from an arm. This syntax and exhaustiveness policy are experimental.

## Diagnostics and commands

`tok check file.tok` lexes, parses, and type-checks; `tok run file.tok` additionally evaluates `main()` in the reference interpreter. A run requires a parameterless, non-generic `main`. `tok build file.tok -o output` produces a host executable through the experimental Rust bootstrap. `--json` before the path emits a JSON result or diagnostic with byte span, line, and column. Diagnostics use `E001` invalid character, `E002` parse error, `E003` integer literal range, `E004` invalid string literal, `E101` unknown name, `E102` type mismatch, `E103` unknown type, `E104` invalid operands, `E105` arity mismatch, `E106` duplicate name, `E107` unreachable code, `E109` immutable assignment, `E110` invalid array operation, `E111` invalid error propagation, `E112` recursive record, `E113` invalid field access, `E114` invalid enum variant, `E115` type inference failure, `E116` invalid match pattern or coverage, `E201` arithmetic failure, `E202` call-depth limit, `E203` invalid entry point, `E204` interpreter invariant failure, and `E205` array index out of bounds. Error output contains a source line and column. GC, standard library, imports, payload enums, and trait-constrained generics do not yet exist in this prototype.

## Reproduce

On a host with a Rust toolchain and linker:

```text
cargo test --workspace
cargo run -p tokit-compiler --bin tok -- run examples/answer.tok
```

The repository pins Rust 1.98.1. The current Windows development machine uses `cargo +stable-x86_64-pc-windows-gnu` because no MSVC linker is installed. This is a local toolchain choice, not a project ABI decision.
