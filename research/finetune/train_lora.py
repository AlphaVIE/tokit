"""LoRA supervised fine-tuning on the Tokit dataset (chat JSONL).

Requires a CUDA GPU and: pip install -r research/finetune/requirements.txt
Defaults fit Qwen2.5-Coder-7B-Instruct with 4-bit QLoRA on a single 24 GB GPU.

Example:
  python research/finetune/train_lora.py --data data/tokit --out runs/qwen7b-tokit
Then serve the merged model (for example with vLLM) and run evaluate.py with
--provider openai --base-url http://localhost:8000/v1.
"""

from __future__ import annotations

import argparse
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--data", type=Path, required=True, help="directory with train.jsonl and val.jsonl")
    parser.add_argument("--model", default="Qwen/Qwen2.5-Coder-7B-Instruct")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--epochs", type=float, default=3.0)
    parser.add_argument("--lr", type=float, default=2e-4)
    parser.add_argument("--rank", type=int, default=32)
    parser.add_argument("--batch", type=int, default=4)
    parser.add_argument("--grad-accum", type=int, default=4)
    parser.add_argument("--max-length", type=int, default=2048)
    parser.add_argument("--no-4bit", action="store_true", help="train in bf16 instead of 4-bit QLoRA")
    parser.add_argument("--merge", action="store_true", help="also save a merged full model for serving")
    args = parser.parse_args()

    import torch
    from datasets import load_dataset
    from peft import LoraConfig, prepare_model_for_kbit_training
    from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig
    from trl import SFTConfig, SFTTrainer

    dataset = load_dataset("json", data_files={"train": str(args.data / "train.jsonl"),
                                               "validation": str(args.data / "val.jsonl")})
    # Only the chat messages are trained on; metadata stays out of the prompt.
    dataset = dataset.map(lambda row: {"messages": row["messages"]}, remove_columns=["meta"])
    tokenizer = AutoTokenizer.from_pretrained(args.model)
    quantization = None if args.no_4bit else BitsAndBytesConfig(
        load_in_4bit=True, bnb_4bit_quant_type="nf4", bnb_4bit_compute_dtype=torch.bfloat16,
        bnb_4bit_use_double_quant=True)
    model = AutoModelForCausalLM.from_pretrained(args.model, torch_dtype=torch.bfloat16,
                                                 quantization_config=quantization, device_map="auto")
    if quantization is not None:
        model = prepare_model_for_kbit_training(model)
    lora = LoraConfig(r=args.rank, lora_alpha=args.rank * 2, lora_dropout=0.05, task_type="CAUSAL_LM",
                      target_modules=["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"])
    config = SFTConfig(
        output_dir=str(args.out),
        num_train_epochs=args.epochs,
        learning_rate=args.lr,
        per_device_train_batch_size=args.batch,
        gradient_accumulation_steps=args.grad_accum,
        lr_scheduler_type="cosine",
        warmup_ratio=0.03,
        logging_steps=10,
        eval_strategy="epoch",
        save_strategy="epoch",
        bf16=True,
        max_length=args.max_length,
        assistant_only_loss=True,  # learn the answers, not the prompts
        report_to="none",
    )
    trainer = SFTTrainer(model=model, args=config, train_dataset=dataset["train"],
                         eval_dataset=dataset["validation"], processing_class=tokenizer, peft_config=lora)
    trainer.train()
    trainer.save_model(str(args.out / "adapter"))
    tokenizer.save_pretrained(str(args.out / "adapter"))
    if args.merge:
        from peft import AutoPeftModelForCausalLM
        merged = AutoPeftModelForCausalLM.from_pretrained(str(args.out / "adapter"), torch_dtype=torch.bfloat16)
        merged = merged.merge_and_unload()
        merged.save_pretrained(str(args.out / "merged"), safe_serialization=True)
        tokenizer.save_pretrained(str(args.out / "merged"))


if __name__ == "__main__":
    main()
