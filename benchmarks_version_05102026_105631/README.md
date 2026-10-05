# Function patch payload experiment

Three frozen LF-normalized source snapshots from main commit `c889b1b` cover
a small arithmetic program, the versioned wide-integer benchmark, and the
JSON library. Changes are subtraction instead of addition, a one-unit sum
offset, and uppercase `F` in a JSON Unicode escape. The contract fixes exact
before/after output. These changes are synthetic maintenance edits, not
improvements to the original programs.

The runner verifies original execution, index spans and hashes, a read-only
preview, the exact written source, and patched execution. A byte splice
computed independently from the index is compared with both patch results.
It reports whole-file output, the complete compact JSON patch request,
the full index, one selected index entry, and a three-context-line unified
diff. JSON escaping, SHA-256 guards, and field names are included. The diff
is a text-size comparator; it is not applied by this runner.

```text
.venv/Scripts/python.exe scripts/benchmark_function_patches.py benchmarks_version_05102026_105631 --tok target/debug/tok.exe --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json --output benchmarks_version_05102026_105631/results.json
```

Use `--verify-only` without tokenizer/output arguments for behavioral checks.
CI runs that mode. Tokenizer versions, the pinned Qwen artifact, source hashes,
payload hashes, and compiler binary hash are recorded in `results.json`.

Each cell is cl100k / o200k / Qwen tokens:

| Case | Full source | Patch request | Full index | Selected index entry | Unified diff |
| --- | ---: | ---: | ---: | ---: | ---: |
| Small | 45 / 45 / 51 | 72 / 73 / 106 | 103 / 104 / 164 | 45 / 45 / 77 | 83 / 84 / 92 |
| Wide sum | 151 / 150 / 160 | 137 / 138 / 164 | 112 / 113 / 164 | 54 / 54 / 78 | 242 / 242 / 255 |
| JSON | 2744 / 2776 / 2983 | 163 / 164 / 191 | 1372 / 1380 / 1982 | 53 / 53 / 78 | 398 / 399 / 423 |

The patch is larger than full output for the small case. For wide sum, its
small output saving disappears when an index entry must also be delivered;
Qwen already counts more patch tokens than full-file tokens. The JSON case
has a substantial output saving, including when the full index is added.
The selected-entry figure assumes the client already knows which function
to request; the current CLI emits the full index.

These are payload counts, not total agent cost or generation accuracy.
They exclude prompts, source retrieval, CLI transport, retries and tool
responses. Adding component counts does not reproduce tokenization of a
complete conversation. The three hand-selected edits cannot establish
general savings, and minified source makes line-based diffs relatively large.
