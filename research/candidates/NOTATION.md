# Provisional notation key

All three files implement [the seed fixture](../fixture.md). Their `.tok.txt` suffix prevents an experimental grammar from being mistaken for accepted Tokit.

## A

`use`, `struct`, `fn`, `let`, `for`, and `if` have their apparent meanings. `[T]` is an array, `Result<T,E>` is a typed result, and postfix `?` propagates an error. A block's last expression is its result. `||` creates a zero-argument closure.

## B

Prefix forms have fixed arity: `~` imports, `#` declares a structure, `^` declares a function (or anonymous function with empty parameter list), `=` binds, `:=` reassigns, `*` loops, `%` branches, `+` appends to an array, `.` projects a field, `!` propagates a typed error, `?` constructs a result type, and `r` returns early. The last expression of `^` is returned. Ordinary function calls are prefix forms. `[T]` is an array type; `[]` is an empty array value. The punctuation is deliberately unoptimized and may tokenize badly.

## C

Each row is `@node-id|opcode|operands`. `M` imports, `S` defines a structure, `F` defines a function (name, generic parameters, parameters, return type, ordered body references), `V` binds a typed value, `A` reassigns, `L` iterates, `I` branches, `C` calls for side effects, and `R` returns. A function with an expression body may use that expression in the final field. IDs 7–12 and 14–17 form the bodies of functions 6 and 13 in the seed fixture. Expressions inside operand fields are a provisional mini-expression encoding; a true positional design would need to decide whether to give every expression a node ID. This hybrid is a research limitation, not a hidden claim of full AST serialization.

None of these forms specifies ownership, effect checking, serialization derivation, or the runtime behavior of the imported modules. Those are open design questions.
