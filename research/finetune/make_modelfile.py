"""Write an Ollama Modelfile for a fine-tuned Tokit model in GGUF format.

The Modelfile embeds spec/LLM_GUIDE.md as the system prompt (it anchors
builtins that the training data covers only lightly) and uses a low
temperature, which suits code generation with compiler verification.

Example:
  python research/finetune/make_modelfile.py --gguf runs/qwen7b-tokit/tokit-7b-q5_k_m.gguf --out runs/qwen7b-tokit/Modelfile
  ollama create tokit-7b -f runs/qwen7b-tokit/Modelfile
"""

from __future__ import annotations

import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def modelfile(gguf: Path, guide: str, temperature: float, context: int) -> str:
    if '"""' in guide:
        raise ValueError("the guide must not contain triple quotes")
    return (
        f"FROM {gguf.resolve().as_posix()}\n"
        f"PARAMETER temperature {temperature}\n"
        f"PARAMETER num_ctx {context}\n"
        'SYSTEM """You write Tokit, a token-efficient, statically typed programming language. '
        "Answer with one complete canonical Tokit program in a ```tokit code block.\n\n"
        f'{guide.strip()}\n"""\n'
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--gguf", type=Path, required=True, help="quantized GGUF model file")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--guide", type=Path, default=ROOT / "spec" / "LLM_GUIDE.md")
    parser.add_argument("--temperature", type=float, default=0.2)
    parser.add_argument("--context", type=int, default=8192)
    args = parser.parse_args()
    text = modelfile(args.gguf, args.guide.read_text(encoding="utf-8"), args.temperature, args.context)
    args.out.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {args.out}; next: ollama create tokit -f {args.out}")


if __name__ == "__main__":
    main()
