# Self-hosting bootstrap

The compiler is written in Rust. Self-hosting proceeds one component at a
time: a Tokit implementation joins the repository only together with an
equivalence test against the Rust component it mirrors.

## Lexer

[`selfhost/lexer.tok`](../selfhost/lexer.tok) is Tokit's lexer written in
Tokit. It reads source text from standard input and prints one
`Kind start end` line per token, using byte offsets and ending with `Eof`.
On invalid input it prints `error CODE start end` and exits with status 1.
`tok tokens file.tok` prints the same listing from the Rust lexer.

```text
tok tokens examples/answer.tok
tok run selfhost/lexer.tok < examples/answer.tok
```

[`compiler/tests/selfhost_lexer.rs`](../compiler/tests/selfhost_lexer.rs)
runs the Tokit lexer in the reference interpreter and, when `rustc` is
available, as a native executable. Each listing and exit status must equal
the Rust lexer's for every `.tok` file under `examples/` and `selfhost/`
(including the lexer itself), for each lexer error code (`E001`, `E003`,
`E004`), and for a dense mix of operators, literals, comments, and Unicode.

The Tokit lexer uses only the public language: `Bytes` indexing, records,
string patterns, `Result` with `?`, and standard input and output. It does
not yet produce token values (identifier text, decoded strings), which the
parser stage will need next.

## Next components

1. Token values and a self-hosted parser for the expression subset, checked
   against `tok fmt` round trips.
2. A self-hosted formatter, checked against `tok fmt` over the corpus.
3. Type checking, which needs the parser and a symbol table first.
