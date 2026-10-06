# Teaching models to write Tokit

This directory holds a verified dataset, a LoRA training script, and a
compiler-judged evaluation harness. Every number a model scores here comes
from `tok`, not from string similarity.

## Dataset

`data/` was produced by `generate_dataset.py` with the release compiler
(`stats.json` records its hash and the seed):

| Split | Rows | Generation | Repair | Purpose |
| --- | ---: | ---: | ---: | --- |
| `train.jsonl` | 1,088 | 842 | 246 | supervised fine-tuning |
| `val.jsonl` | 60 | 44 | 16 | early stopping |
| `test_iid.jsonl` | 61 | 45 | 16 | unseen variants of seen task families |
| `test_ood.jsonl` | 275 | 200 | 75 | five task families never seen in training |

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

## Fine-tune an open model

```text
pip install -r research/finetune/requirements.txt
python research/finetune/train_lora.py --data research/finetune/data --out runs/qwen7b-tokit --merge
vllm serve runs/qwen7b-tokit/merged --served-model-name tokit-7b
```

Defaults: Qwen2.5-Coder-7B-Instruct, 4-bit QLoRA, rank 32, 3 epochs,
learning rate 2e-4, loss on assistant turns only; one 24 GB GPU suffices.

## Recommendation

1. **Start without fine-tuning.** Give a frontier model (Claude, GPT/Codex)
   `spec/LLM_GUIDE.md` (about 2,000 tokens) as its system prompt and let it
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
   `train_lora.py`, serve it, and compare against step 1 on `test_ood` with
   the same harness. Keep the guide in the system prompt at inference even
   after fine-tuning; it costs little and anchors newer builtins.

The template dataset is intentionally narrow: it teaches syntax, canonical
form, builtins, and repair from diagnostics, not open-ended software design.
Treat `test_ood` as the honest generalization signal.
