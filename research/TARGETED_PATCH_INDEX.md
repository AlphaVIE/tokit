# Targeted patch index response

The [function patch corpus](../benchmarks_version_05102026_105631/README.md)
showed that returning every function hash creates substantial input overhead.
The CLI now accepts a target name. The response retains the version and
functions array, but contains only the uniquely named function.

Measured on the same frozen sources and pinned tokenizers as the original
experiment. Each cell is cl100k / o200k / Qwen tokens, including the complete
JSON response envelope:

| Case | Full index | Targeted response |
| --- | ---: | ---: |
| Small | 103 / 104 / 164 | 52 / 53 / 84 |
| Wide sum | 112 / 113 / 164 | 61 / 62 / 85 |
| JSON | 1372 / 1380 / 1982 | 60 / 61 / 85 |

The runner checks that the targeted entry equals the corresponding full-index
entry before validating the patch's preview, file content, and execution.
CI exercises all three cases. Missing and duplicate target names are rejected
by dedicated compiler tests.

```text
.venv/Scripts/python.exe scripts/benchmark_function_patches.py benchmarks_version_05102026_105631 --tok target/debug/tok.exe --qwen-tokenizer research/.cache/qwen2.5-coder-tokenizer.json --targeted-index --output research/results/targeted_patch_index.json
```

[Raw results](results/targeted_patch_index.json) record source, payload, compiler
and tokenizer hashes. This reduces retrieval output when the caller already
knows the target name. Target discovery, source context, prompts, retries and
model accuracy remain outside the measurement. It does not change the patch
request cost or establish total agent savings. Small-file patch overhead
still exceeds full-source output in the original experiment.
