# Teaching models to write Tokit

This directory holds a verified dataset, a LoRA training script, and a
compiler-judged evaluation harness. Every number a model scores here comes
from `tok`, not from string similarity.

## Dataset

`data/` was produced by `generate_dataset.py` with the release compiler
(`stats.json` records its hash and the seed):

| Split | Rows | Generation | Repair | Purpose |
| --- | ---: | ---: | ---: | --- |
| `train.jsonl` | 1,398 | 1,068 | 330 | supervised fine-tuning |
| `val.jsonl` | 78 | 60 | 18 | early stopping |
| `test_iid.jsonl` | 78 | 49 | 29 | unseen variants of seen task families |
| `test_ood.jsonl` | 275 | 200 | 75 | five task families never seen in training |

The 39 task families cover the core language and newer features: nested
patterns, structural `==`, maps, hashing, bit operations, and stateful and
worker-pool HTTP servers. Repair mutations include method-call syntax
(`x.len()`, `x.to_string()`), unqualified enum variants, `->` instead of
`=>`, and Python operators.

Each row is chat JSONL (`messages` with system, user, assistant) plus
`meta` (family, kind, expected output, arguments, stdin). Generation rows
ask for a program and answer with canonical Tokit whose `tok run` output
equals an independent Python reference. Repair rows show a mutated program
with its real `tok check` diagnostic and answer with the fixed program.
Regenerate or enlarge it with:

```text
python research/finetune/generate_dataset.py --tok target/release/tok --out research/finetune/data --per-family 40 --repairs-per-family 15
```

## Evaluate any model

```text
# Self-test of the judge (must print pass@1 = 1.0):
python research/finetune/evaluate.py --data research/finetune/data/test_ood.jsonl --tok target/release/tok --provider oracle --out runs/oracle.jsonl

# Claude with the language guide as system prompt and two compiler-feedback rounds:
python research/finetune/evaluate.py --data research/finetune/data/test_ood.jsonl --tok target/release/tok \
  --provider anthropic --model claude-opus-5-5 --system-file spec/LLM_GUIDE.md --repair-rounds 2 --out runs/claude.jsonl

# Any OpenAI-compatible endpoint (OpenAI, or vLLM serving your fine-tuned model):
python research/finetune/evaluate.py --data research/finetune/data/test_ood.jsonl --tok target/release/tok \
  --provider openai --base-url http://localhost:8000/v1 --model tokit-7b --repair-rounds 2 --out runs/tokit7b.jsonl
```

The summary reports `pass@1` (first answer correct), `compile@1`, the pass
rate after the repair rounds, and token usage.

## Fine-tune an open model on a 16 GB GPU

Tested settings target one 16 GB card such as an RTX 5060 Ti; peak memory is
about 11–13 GB. The longest training sample is under 1,024 tokens.

1. **Environment** (Windows natively or WSL2). RTX 50-series cards need
   PyTorch built for CUDA 12.8 or newer:

   ```text
   python -m venv .venv-ft && .venv-ft\Scriptsctivate
   pip install torch --index-url https://download.pytorch.org/whl/cu128
   pip install -r research/finetune/requirements.txt
   ```

2. **Train** Qwen2.5-Coder-7B-Instruct with 4-bit QLoRA (rank 16, batch 4 ×
   accumulation 4, 1,024 tokens, gradient checkpointing, paged 8-bit Adam).
   Three epochs over the 1,398 training rows take roughly 30–60 minutes:

   ```text
   python research/finetune/train_lora.py --data research/finetune/data --out runs/qwen7b-tokit --merge
   ```

   `--merge` writes a full bf16 model to `runs/qwen7b-tokit/merged`, merged
   on the CPU (needs about 16 GB of system RAM). If memory runs short, use
   `--batch 2 --grad-accum 8`. The 14B model
   (`--model Qwen/Qwen2.5-Coder-14B-Instruct --batch 1 --grad-accum 16`) fits
   only tightly and is worth trying after 7B works.

3. **Quantize and serve with Ollama.** A merged 7B model needs about 15 GB in
   bf16, too much to serve comfortably on 16 GB, so convert it to GGUF with
   [llama.cpp](https://github.com/ggml-org/llama.cpp) and quantize to Q5_K_M
   (about 5.4 GB):

   ```text
   python llama.cpp/convert_hf_to_gguf.py runs/qwen7b-tokit/merged --outfile runs/qwen7b-tokit/tokit-7b-f16.gguf
   llama.cpp/build/bin/llama-quantize runs/qwen7b-tokit/tokit-7b-f16.gguf runs/qwen7b-tokit/tokit-7b-q5_k_m.gguf Q5_K_M
   python research/finetune/make_modelfile.py --gguf runs/qwen7b-tokit/tokit-7b-q5_k_m.gguf --out runs/qwen7b-tokit/Modelfile
   ollama create tokit-7b -f runs/qwen7b-tokit/Modelfile
   ```

   The Modelfile embeds `spec/LLM_GUIDE.md` as the system prompt and sets a
   low temperature.

4. **Evaluate** through Ollama's OpenAI-compatible endpoint, with and without
   compiler feedback, and compare with the untuned base model:

   ```text
   python research/finetune/evaluate.py --data research/finetune/data/test_ood.jsonl --tok target/release/tok      --provider openai --base-url http://localhost:11434/v1 --model tokit-7b --repair-rounds 2 --out runs/tokit7b.jsonl
   ```

## Recommendation

1. **Start without fine-tuning.** Give a frontier model (Claude, GPT/Codex)
   `spec/LLM_GUIDE.md` (about 2,500 tokens) as its system prompt and let it
   call `tok check --json` in a loop, as `--repair-rounds` does. Tokit's
   diagnostics name the code, location, and expected versus actual type, so
   most first-attempt errors are repaired in one round. Measure this baseline
   on `test_ood` first; it is cheap and often sufficient.
2. **Distill for a small, fast model.** If you need a local or cheap model,
   generate many diverse tasks, solve them with the frontier model plus the
   compiler loop, keep only programs that `tok` verifies, and add them to
   `train.jsonl`. This rejection-sampled data is broader than the template
   families and is the main lever for quality. Mix in about 10–20% general
   code data to avoid forgetting.
3. **Then fine-tune** Qwen2.5-Coder (7B for speed, 14B/32B for quality) with
   `train_lora.py`, serve it through Ollama, and compare against step 1 on
   `test_ood` with the same harness. Keep the guide in the system prompt at inference even
   after fine-tuning; it costs little and anchors newer builtins.

The template dataset is intentionally narrow: it teaches syntax, canonical
form, builtins, and repair from diagnostics, not open-ended software design.
Treat `test_ood` as the honest generalization signal.
