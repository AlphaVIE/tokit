# Experimental candidate A subset (not a stable Tokit specification)

This document describes exactly what the current Rust prototype accepts. It is a test vehicle for syntax and compiler architecture. The canonical language grammar has **not** been selected.

## Accepted constructs

```text
program  = function+ ;
function = "fn" identifier "(" parameters? ")" "->" type block ;
parameters = identifier ":" type ("," identifier ":" type)* ;
type     = "i32" | "bool" | "Unit" ;
block    = "{" statement* expression? "}" ;
statement = "let" identifier ":" type "=" expression ";"
          | "return" expression ";"
          | expression ";" ;
expression = integer | "true" | "false" | identifier
           | identifier "(" arguments? ")"
           | "(" expression ")" | block
           | "if" expression block "else" (block | expression-if)
           | expression binary-op expression ;
binary-op = "+" | "-" | "*" | "/" | "==" | "!=" | "<" | "<=" | ">" | ">=" ;
```

Binary operators use normal arithmetic precedence, with equality below comparisons. Blocks return their final expression; a block without a final expression has type `Unit`. `return` exits the current function. `if` requires `else`, and both branches must have compatible types. Functions may call later functions and recurse. Local bindings require explicit types and are immutable; shadowing an outer binding in a nested block is currently permitted, while duplicate names in one block are rejected.

`i32` arithmetic is checked in the interpreter: overflow and division by zero produce `E201`. Integer literals must fit `i32`. This subset has no unary minus, so negative literals cannot currently be written directly; subtraction can produce negative values. These are prototype limitations, not final numeric semantics.

## Diagnostics and commands

`tok check file.tok` lexes, parses, and type-checks; `tok run file.tok` additionally evaluates `main()` in the reference interpreter. A run requires a parameterless `main`. `--json` before the path emits a JSON result or diagnostic with byte span, line, and column. Diagnostics use `E001` invalid character, `E002` parse error, `E003` literal range, `E101` unknown name, `E102` type mismatch, `E104` invalid operands, `E105` arity mismatch, `E106` duplicate name, `E107` unreachable code, `E201` arithmetic failure, `E202` call-depth limit, and `E203` invalid entry point. Error output contains a source line and column. No binary, native compilation, GC, standard library, imports, arrays, structs, or generics exist in this prototype.

## Reproduce

On a host with a Rust toolchain and linker:

```text
cargo test --workspace
cargo run -p tokit-compiler --bin tok -- run examples/answer.tok
```

The repository pins Rust 1.98.1. The current Windows development machine uses `cargo +stable-x86_64-pc-windows-gnu` because no MSVC linker is installed. This is a local toolchain choice, not a project ABI decision.
