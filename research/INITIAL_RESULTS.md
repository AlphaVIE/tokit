# Initial source-token measurements

Initial run on 2026-09-29 with Python 3.11.9 and `tiktoken==0.14.0`:

```powershell
python -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r requirements-research.txt
.\.venv\Scripts\python.exe scripts/token_cost.py research/candidates
```

| Candidate | UTF-8 bytes | Characters | cl100k_base tokens | o200k_base tokens |
| --- | ---: | ---: | ---: | ---: |
| A — conventional | 512 | 512 | 133 | 134 |
| B — symbolic | 519 | 519 | 144 | 144 |
| C — positional | 531 | 531 | 221 | 229 |

All files contain ASCII only and LF line endings. On this single seed fixture, A has the lowest source-token count in both tested encodings. C's repeated IDs and separators cost substantially more tokens. B's symbols do not automatically save tokens. These are observations about the current examples, **not** evidence that A is the best language design. The examples are unparsed, the standard APIs are abstract, and both tokenizers come from the same library family.

The follow-up experiment added five fixtures and a Qwen tokenizer. See [CROSS_FAMILY_RESULTS.md](CROSS_FAMILY_RESULTS.md). A is the practical starting point for the first parser prototype because its source-token count is currently favorable and its syntax can be parsed with familiar techniques. This is a provisional engineering recommendation, not a grammar commitment.
