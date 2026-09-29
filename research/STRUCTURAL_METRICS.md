# Structural density measurements

The experimental `tok stats file.tok` command type-checks a complete Tokit source file and emits JSON containing UTF-8 bytes, Unicode scalar-value characters, AST nodes, semantic operations, functions, top-level declarations, and dependencies. It reports zero dependencies because this compiler subset has no imports. The command does not tokenize source for any LLM model.

Run the multi-tokenizer join on executable Tokit sources with:

```powershell
cargo build -p tokit-compiler --bin tok
.venv\Scripts\python.exe scripts/token_cost.py research/baselines --tok-stats-binary target/debug/tok.exe --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json
```

On Unix, use the appropriate Python and `tok` paths. The script measures each entire `.tok` file, including benchmark marker comments, and calculates tokens per AST node, semantic operation, function, declaration, and dependency for each tokenizer. A zero denominator yields JSON `null`. It verifies that the compiler and tokenizer saw identical byte and character counts. The core regions between markers are useful for syntax comparisons but are not independently executable programs, so structural density uses whole checked files.

## Counting rules

An AST node is the program root, each top-level declaration, field declaration, enum variant, parameter declaration, type occurrence (including nested type arguments), statement, expression, and match pattern. Identifier strings, operators, and generic parameter names are attributes of those nodes and are not counted separately. A semantic operation is an array literal, index, field read, enum or result construction, `?`, binary expression, call or record construction, branch, match, binding, assignment, loop, or return. Literals, variable reads, expression statements, and blocks add no semantic operation. These counts describe *static source structure*, not executed instructions or runtime work. The counting rule is versioned by the compiler implementation; changes must be called out before comparing historical results.

## Initial executable Tokit observations

Measured on 2026-09-29 using the three pinned tokenizer configurations documented in [CROSS_FAMILY_RESULTS.md](CROSS_FAMILY_RESULTS.md). These are whole-file measurements, including marker comments.

| Fixture | AST nodes | Operations | cl100k / o200k / Qwen tokens | cl100k tokens per AST node | cl100k tokens per operation |
| --- | ---: | ---: | ---: | ---: | ---: |
| Checked division | 24 | 6 | 54 / 53 / 57 | 2.25 | 9.00 |
| Generic pair | 18 | 3 | 38 / 38 / 38 | 2.11 | 12.67 |
| Sum positive | 25 | 6 | 52 / 51 / 55 | 2.08 | 8.67 |

These ratios are within-Tokit diagnostics. They do not demonstrate a token advantage over other languages: equivalent executable programs currently have different wrappers, and AST/operation counts need a shared cross-language definition before normalizing them. Model generation, repair, performance, and success-rate measurements remain separate work.
