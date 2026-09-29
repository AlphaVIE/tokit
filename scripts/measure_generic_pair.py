"""Measure marked core definitions in generic-pair baselines."""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path

import tiktoken

from token_cost import QWEN_REVISION, QWEN_SHA256, measure


BASE = Path(__file__).resolve().parents[1] / "research" / "baselines" / "generic-pair"
FILES = ("tokit.tok", "rust.rs", "go.go", "c.c", "cpp.cpp", "python.py", "typescript.ts", "Main.java")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qwen-tokenizer", type=Path)
    args = parser.parse_args()
    encodings = {name: tiktoken.get_encoding(name).encode for name in ("cl100k_base", "o200k_base")}
    artifacts: dict[str, object] = {}
    if args.qwen_tokenizer:
        from tokenizers import Tokenizer

        digest = hashlib.sha256(args.qwen_tokenizer.read_bytes()).hexdigest()
        if digest != QWEN_SHA256:
            parser.error(f"Qwen tokenizer SHA-256 mismatch: {digest}")
        tokenizer = Tokenizer.from_file(str(args.qwen_tokenizer))
        label = "Qwen2.5-Coder-0.5B-Instruct"
        encodings[label] = lambda source: tokenizer.encode(source, add_special_tokens=False).ids
        artifacts[label] = {"revision": QWEN_REVISION, "sha256": digest,
                            "tokenizers_version": importlib.metadata.version("tokenizers")}
    measurements = []
    for name in FILES:
        item = measure(BASE / name, encodings, "core")
        item["file"] = f"research/baselines/generic-pair/{name}"
        measurements.append(item)
    result = {"fixture": "generic-pair", "region": "core",
              "tiktoken_version": importlib.metadata.version("tiktoken"),
              "artifacts": artifacts, "measurements": measurements}
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
