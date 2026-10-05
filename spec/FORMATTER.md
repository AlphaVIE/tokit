# Experimental canonical formatter

`tok compact file.tok` removes optional `fn` keywords from function
declarations and shortens parsed `i32`/`i64` type names to `I`/`L`.
`tok compact --write file.tok` applies both changes to the file.
`--functions-only` and `--types-only` select one transformation for isolated
measurements. The type transformation leaves names in expressions, conversion
calls, literal suffixes, strings, and comments untouched. Both transformations
are deterministic and idempotent.

`tok fmt file.tok` prints a canonical whitespace form. `tok fmt --check file.tok` exits unsuccessfully when the file differs; `tok fmt --write file.tok` replaces it. The formatter parses the input first, preserves every lexer token and line comment, removes unnecessary whitespace, separates top-level declarations by one newline, and writes a final newline. It inserts a space only when concatenating adjacent token spellings would change tokenization. It moves line comments to their own lines so that surrounding syntax remains unchanged.

The output is deterministic and idempotent for the current experimental grammar. Tests compare lexer token sequences before and after formatting across all `examples/*.tok` files and the executable Tokit baseline sources. The command checks syntax but does not require the program to type-check; formatting does not execute code.

This is a **whitespace canonicalization experiment**, not a final source convention. It retains original string escape spellings, integer literal spellings, declaration order, and equivalent expression forms. Comments are retained as text, but their exact horizontal placement is not. Future AST-level normalization needs semantic equivalence checks and token-cost measurements before replacing this format.
