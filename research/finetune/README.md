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

Measured on an RTX 5060 Ti 16 GB under Windows: peak GPU memory 11.9 GB,
about 7 seconds per optimizer step, so three epochs take roughly half an
hour. The longest training sample is under 1,024 tokens.

1. **Environment** (Windows natively or WSL2). RTX 50-series cards need
   PyTorch built for CUDA 12.8 or newer:

   ```text
   python -m venv .venv-ft && .venv-ft\Scriptsctivate
   pip install torch --index-url https://download.pytorch.org/whl/cu128
   pip install -r research/finetune/requirements.txt
   ```

2. **Train** Qwen2.5-Coder-7B-Instruct with 4-bit QLoRA (rank 16, batch 4 ×
   accumulation 4, 1,024 tokens, gradient checkpointing, paged 8-bit Adam).
   Training uses prompt/completion pairs, so only the answers carry loss:

   ```text
   python research/finetune/train_lora.py --data research/finetune/data --out runs/qwen7b-tokit --merge
   ```

   `--merge` writes a full bf16 model to `runs/qwen7b-tokit/merged`, merged
   on the CPU (needs about 16 GB of system RAM). If memory runs short, use
   `--batch 2 --grad-accum 8`. The 14B model
   (`--model Qwen/Qwen2.5-Coder-14B-Instruct --batch 1 --grad-accum 16`) fits
   only tightly and is worth trying after 7B works.

3. **Quantize and serve with Ollama.** A merged 7B model needs about 15 GB
   in bf16, too much to serve comfortably on 16 GB, so convert it to GGUF and
   quantize it to Q5_K_M (5.4 GB). Ollama 0.35 cannot import Qwen2
   safetensors directly, so use [llama.cpp](https://github.com/ggml-org/llama.cpp)'s
   converter (in its own virtual environment: its requirements pin older
   `transformers`) and a prebuilt `llama-quantize` from its releases:

   ```text
   git clone --depth 1 https://github.com/ggml-org/llama.cpp
   python llama.cpp/convert_hf_to_gguf.py runs/qwen7b-tokit/merged --outfile runs/qwen7b-tokit/tokit-7b-f16.gguf --outtype f16
   llama-quantize runs/qwen7b-tokit/tokit-7b-f16.gguf runs/qwen7b-tokit/tokit-7b-q5_k_m.gguf Q5_K_M
   python research/finetune/make_modelfile.py --gguf runs/qwen7b-tokit/tokit-7b-q5_k_m.gguf --out runs/qwen7b-tokit/Modelfile
   ollama create tokit-7b -f runs/qwen7b-tokit/Modelfile
   ```

   Converting takes about a minute and quantizing another. The Modelfile
   embeds `spec/LLM_GUIDE.md` as the system prompt and sets a low
   temperature.

4. **Evaluate** through Ollama's OpenAI-compatible endpoint, with and without
   compiler feedback, and compare with the untuned base model:

   ```text
   python research/finetune/evaluate.py --data research/finetune/data/test_ood.jsonl --tok target/release/tok      --provider openai --base-url http://localhost:11434/v1 --model tokit-7b --repair-rounds 2 --out runs/tokit7b.jsonl
   ```

## First local result

Qwen2.5-Coder-7B-Instruct, trained with the defaults above on an RTX 5060 Ti
16 GB (27.7 minutes, 3 epochs), served through Ollama at Q5_K_M, and
evaluated on `test_ood` (275 rows, five task families never seen in
training) with up to two compiler-feedback rounds. The base model received
`spec/LLM_GUIDE.md` as its system prompt; the fine-tuned model only the
dataset's one-line prompt.

| Model | pass@1 | compile@1 | after repair | input tokens |
| --- | ---: | ---: | ---: | ---: |
| Qwen2.5-Coder-7B + guide | 23.6% | 29.5% | 25.1% | 1,992,441 |
| **tokit-7b (fine-tuned)** | **49.1%** | **50.2%** | **50.2%** | **102,873** |

Per family, generation / repair rows passed:

| Family | base + guide | tokit-7b |
| --- | --- | --- |
| binary_search | 40/40 · 14/15 | 40/40 · 15/15 |
| dispatch | 1/40 · 0/15 | 30/40 · 15/15 |
| rle | 0/40 · 10/15 | 0/40 · 15/15 |
| tree | 0/40 · 0/15 | 0/40 · 15/15 |
| brackets | 0/40 · 4/15 | 0/40 · 8/15 |

Fine-tuning doubles the pass rate at a twentieth of the prompt tokens and
makes repair from diagnostics nearly reliable (68 of 75 repair rows). It
does not yet generalize to unseen problem shapes: on `rle`, `tree`, and
`brackets` both models fall back to Rust idioms (`let mut`, `char`, `&mut`,
`pop`). Broadening the training data with verified, model-generated
programs (recommendation 2 below) is the next lever; validation loss near
zero shows the template families alone are learned completely.

## Second run: stack families

After `last` and `pop` were added to the language, the dataset gained four
training families (`rpn`, `undo`, `dedupe`, `expr_tree`) and repair
mutations for the Rust habits seen above (`let mut`, `char`, `.last()`,
`let top=xs.pop();`), giving 1,596 training rows. The held-out families are
unchanged except that the `brackets` reference now uses `last`/`pop`. All
three models were evaluated on the regenerated `test_ood`:

| Model | pass@1 | after repair | input tokens |
| --- | ---: | ---: | ---: |
| Qwen2.5-Coder-7B + guide | 21.8% | 22.9% | 2,045,128 |
| tokit-7b (first dataset) | 46.2% | **47.6%** | 109,032 |
| tokit-7b-v2 (stack families) | 34.5% | 35.6% | 126,588 |

The second model is worse on unseen families: `binary_search` generation
fell from 40/40 to 19/40 and `dispatch` from 26/40 to 5/40, while
`brackets` stayed at 0/40 despite `pop`. Its failures are ordinary
generation mistakes (a stray `;` after `Some(lo)`, `char` types, `max` over a
list) that two repair rounds did not fix. More template families do not
improve generalization; with validation loss near zero in both runs, the
model memorizes the templates. Keep the first model, and broaden the data
with verified programs from a stronger model (recommendation 2) before
training again.

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
