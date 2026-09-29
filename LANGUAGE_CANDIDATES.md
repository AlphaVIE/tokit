# Three experimental source representations

The files in `research/candidates/` express the same seed program under three **invented** notations. They are not valid Tokit and must not be treated as a specification. The behavior contract is in `research/fixture.md`; speculative details are listed below. Each uses explicit primitive types, a generic identity function, a `User` structure, an HTTP handler, JSON, a loop, a branch, a typed error path, a module import, and a spawned task.

## A — compressed conventional text

Named declarations, braces, and infix expressions resemble familiar source while removing some optional words. This candidate emphasizes predictable generation and a small learning gap. It is likely to spend tokens on repeated names and punctuation. Its parser can plausibly use ordinary declaration parsing and a Pratt expression parser.

## B — symbolic prefix machine notation

Single-character operators encode declarations and control flow. Expressions are prefix forms and blocks are parenthesized. It avoids precedence ambiguity and minimizes bytes, but tokenizers may split symbols poorly; model errors and human review cost may be high. Every symbol needs one precise meaning. The current sample is intentionally uncomfortable to read.

## C — positional AST-like records

Each line is a typed node record. Numeric references point to earlier nodes; the top-level graph is explicit. This is closer to a serialized program than source text. It may support exact agent edits and avoid repeated identifier spelling, but reference management, diff stability, and generation errors are real costs. A parser would be simple; validation and stable identity would be harder.

## Questions to settle experimentally

- How do the candidates tokenize in distinct model families on large, equivalent programs?
- Does any reduction in source tokens survive prompt explanation, compiler errors, and repair attempts?
- Which form produces fewer malformed programs and more accurate modifications?
- How costly are canonical formatting and semantic diffs?
- Can a human explanation reliably reconstruct control flow and security-relevant effects?
- Does symbolic notation create accidental meanings from small edits?

The recommendation today is **to keep all three alive**. Initial counts only cover a seed fixture and two OpenAI encodings. They are insufficient to select a grammar. A favorable count would justify broader trials, not a syntax freeze.
