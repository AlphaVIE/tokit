# Research plan and implementation roadmap

## Phases 0–3: determine what to build

| Phase | Work | Artifact / exit condition |
| --- | --- | --- |
| 0. Problem | Define comparable work units, goals, safety constraints | [DESIGN_GOALS.md](DESIGN_GOALS.md), reviewed workloads |
| 1. Tokenizers | Pin versions and model encodings; study identifier and punctuation behavior | Reproducible counts and per-snippet breakdowns |
| 2. Syntax | Maintain three distinct candidates and precise semantic fixtures | Candidate corpus with one behavior contract per fixture |
| 3. Evaluation | Count tokens, test model generation and repairs, prototype parsers | Decision record with uncertainty and rejected alternatives |

The present corpus is a **seed**, not a representative benchmark. Its token counts may reveal obvious costs but cannot decide the language. Expand it with realistic APIs, parsing, file processing, data structures, errors, and maintenance edits. Have independent reviewers check functional equivalence before comparing counts.

## Phases 4–15: implementation after design review

| Stage | Deliverable | Verification |
| --- | --- | --- |
| 4. Subset spec | Lexical grammar, EBNF, types, numeric/error rules | Examples and rejection cases |
| 5. Front end | Rust lexer, parser, AST with spans | Golden tests, parser fuzzing |
| 6. Semantics | Name resolution and static type checking | Positive and negative suites |
| 7. Reference execution | Small interpreter if useful to settle semantics | Cross-check corpus |
| 8. IR | Backend-independent typed IR | IR verifier and lowering tests |
| 9. Native backend | Evaluated LLVM or alternative backend | Differential execution, performance baseline |
| 10. Runtime | Managed allocation prototype and safety boundary | Stress tests and memory measurements |
| 11. Standard library | Core values, files, JSON, networking | Integration tests |
| 12. Toolchain | CLI, formatter, tests, package prototype | Reproducibility and usability tests |
| 13. AI interface | Structured diagnostics, index, transactional AST edits | Round-trip and failure tests |
| 14. IDE | LSP and editor support | Protocol and user workflow tests |
| 15. Self-hosting | Incremental compiler components in Tokit | Bootstrap equivalence |

Each stage records design, implementation, tests, benchmarks, and limitations. The MVP in `PROMPT.txt` spans multiple stages; no single early PR should be presented as that MVP.

## Near-term issue sequence

1. Expand baseline fixtures to Rust, Go, C, C++, Python, TypeScript, and Java.
2. Add tokenizer adapters for at least one non-OpenAI tokenizer family and pin all model files/hashes.
3. Define a fixture schema for semantic equivalence and a review checklist.
4. Prototype small parsers for each representation and measure parse errors and implementation cost.
5. Run model generation, modification, and repair experiments with fixed prompts and validators.
6. Review the first subset specification with the owner before freezing grammar.

## Architectural decisions needing owner input

These are **questions**, not requests to stop the research work:

- What target should define the first runnable milestone: Windows native, Linux native, or both?
- What amount of syntax compression is acceptable if human source review becomes difficult, even with `tok explain`?
- Should the initial public format be solely textual `.tok`, or may a structured patch channel become a supported public interface early?
- Which safety and portability guarantees are release blockers for the first published version?
- Which licensing family should be adopted after a documented comparison (MIT, Apache-2.0, or dual)?

Major commitments remain open until evidence and review. Routine implementation details can be chosen during each stage.
