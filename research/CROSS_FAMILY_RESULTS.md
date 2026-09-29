# Cross-family source-token probe

Measured on 2026-09-29 with Python 3.11.9, `tiktoken==0.14.0`, and `tokenizers==0.23.2`.

The third tokenizer comes from the [Qwen2.5-Coder-0.5B-Instruct model repository](https://huggingface.co/Qwen/Qwen2.5-Coder-0.5B-Instruct) at revision `ea3f2471cf1b1f0db85067f1ef93848e38e88c25`. Its `tokenizer.json` has SHA-256 `c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539` and is loaded with `add_special_tokens=False`. The OpenAI encodings are `cl100k_base` and `o200k_base`; these names and the installed `tiktoken` version identify the local tokenizer implementation. The Qwen tokenizer is a distinct model source, though all three may share broad subword-tokenization ideas.

```powershell
python -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r requirements-research.txt
.\.venv\Scripts\python.exe scripts/fetch_qwen_tokenizer.py
.\.venv\Scripts\python.exe scripts/token_cost.py research/candidates --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json
.\.venv\Scripts\python.exe scripts/token_cost.py research/fixtures --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json
.\.venv\Scripts\python.exe scripts/token_cost.py research/edits --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json
```

The fetch script verifies the digest and stores the 7 MB tokenizer in an ignored cache. The measurement tool emits raw JSON with bytes, characters, tokens, package versions, and artifact hash.

## Tokens by fixture

Each cell lists A / B / C. See [fixture contracts](fixtures/CONTRACTS.md) and the experimental source files under `candidates/` and `fixtures/`.

| Fixture | cl100k_base | o200k_base | Qwen2.5-Coder |
| --- | ---: | ---: | ---: |
| HTTP users | 133 / 144 / 221 | 134 / 144 / 229 | 136 / 147 / 239 |
| Checked division | 33 / 42 / 55 | 33 / 42 / 57 | 36 / 45 / 58 |
| Generic pair | 31 / 39 / 45 | 31 / 39 / 46 | 31 / 39 / 45 |
| Line count | 50 / 56 / 89 | 50 / 56 / 92 | 52 / 58 / 91 |
| Sum positive | 40 / 42 / 72 | 40 / 42 / 75 | 43 / 45 / 75 |
| Task square | 39 / 45 / 59 | 39 / 45 / 61 | 42 / 48 / 62 |
| **Total** | **326 / 368 / 541** | **327 / 368 / 560** | **340 / 382 / 570** |

On these six hand-authored, small fixtures, A uses the fewest source tokens in all three tokenizers. The C sample is a hybrid positional representation with inline expressions; it is not a full AST serialization. The fixtures have not been parsed or executed, some safety semantics are unspecified, and no model generation or repair runs were performed. The totals reflect this corpus only. They cannot establish overall cost, correctness, or performance. The next evidence gate is equivalent executable fixtures and model-generation trials.

The [maintenance-edit probe](edits/filter/CONTRACT.md) changes a count of positive integers into a count of nonnegative integers. Its before/after files are included for all candidates. Full-file tokens increase by 1–2 for the changed source across these encodings; this says nothing yet about actual edit-token cost or model accuracy. Those require a controlled agent trial with the same prompt, patch format, and validator budget.
