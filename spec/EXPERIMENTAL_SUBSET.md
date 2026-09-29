# Experimental candidate A subset (not a stable Tokit specification)

This document describes exactly what the current Rust prototype accepts. It is a test vehicle for syntax and compiler architecture. The canonical language grammar has **not** been selected.

## Accepted constructs

```text
program  = function+ ;
function = "fn" identifier "(" parameters? ")" "->" type block ;
parameters = identifier ":" type ("," identifier ":" type)* ;
type     = "i32" | "bool" | "Unit" | "[" type "]" ;
block    = "{" statement* expression? "}" ;
statement = ("let" | "var") identifier ":" type "=" expression ";"
          | identifier "=" expression ";"
          | "for" identifier "in" expression block
          | "return" expression ";"
          | expression ";" ;
expression = integer | "true" | "false" | identifier | "[" arguments? "]"
           | identifier "(" arguments? ")"
           | "(" expression ")" | block
           | "if" expression block "else" (block | expression-if)
           | expression binary-op expression ;
binary-op = "+" | "-" | "*" | "/" | "==" | "!=" | "<" | "<=" | ">" | ">=" ;
```

Binary operators use normal arithmetic precedence, with equality below comparisons. Blocks return their final expression; a block without a final expression has type `Unit`. `return` exits the current function, including from a loop. `if` requires `else`, and both branches must have compatible types. Functions may call later functions and recurse. Local bindings require explicit types. `let` is immutable and `var` permits reassignment; shadowing an outer binding in a nested block is currently permitted, while duplicate names in one block are rejected. Arrays are homogeneous and can be empty when an expected array type provides context. `for` iterates over an array value; its loop variable is scoped to one iteration. Arrays are copied as values in this interpreter, and indexed access, mutation of array elements, and append are not yet supported.

`i32` arithmetic is checked in the interpreter: overflow and division by zero produce `E201`. Integer literals must fit `i32`. This subset has no unary minus, so negative literals cannot currently be written directly; subtraction can produce negative values. These are prototype limitations, not final numeric semantics.

## Diagnostics and commands

`tok check file.tok` lexes, parses, and type-checks; `tok run file.tok` additionally evaluates `main()` in the reference interpreter. A run requires a parameterless `main`. `--json` before the path emits a JSON result or diagnostic with byte span, line, and column. Diagnostics use `E001` invalid character, `E002` parse error, `E003` literal range, `E101` unknown name, `E102` type mismatch, `E104` invalid operands, `E105` arity mismatch, `E106` duplicate name, `E107` unreachable code, `E109` immutable assignment, `E110` invalid loop iterable, `E201` arithmetic failure, `E202` call-depth limit, `E203` invalid entry point, and `E204` interpreter invariant failure. Error output contains a source line and column. No binary, native compilation, GC, standard library, imports, structs, or generics exist in this prototype.

## Reproduce

On a host with a Rust toolchain and linker:

```text
cargo test --workspace
cargo run -p tokit-compiler --bin tok -- run examples/answer.tok
```

The repository pins Rust 1.98.1. The current Windows development machine uses `cargo +stable-x86_64-pc-windows-gnu` because no MSVC linker is installed. This is a local toolchain choice, not a project ABI decision.
