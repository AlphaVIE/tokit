"""Count pinned-tokenizer costs for equivalent boolean expressions."""
from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path

from token_cost import QWEN_REVISION, QWEN_SHA256


CASES = {
    "and": ("f(a:bool,b:bool)->bool{if a{b}else{false}}", "f(a:bool,b:bool)->bool{a&&b}"),
    "or": ("f(a:bool,b:bool)->bool{if a{true}else{b}}", "f(a:bool,b:bool)->bool{a||b}"),
    "not": ("f(a:bool)->bool{if a{false}else{true}}", "f(a:bool)->bool{!a}"),
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qwen-tokenizer", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    import tiktoken
    from tokenizers import Tokenizer

    actual = hashlib.sha256(args.qwen_tokenizer.read_bytes()).hexdigest()
    if actual != QWEN_SHA256:
        parser.error(f"Qwen tokenizer hash mismatch: {actual}")
    qwen = Tokenizer.from_file(str(args.qwen_tokenizer))
    encoders = {name: tiktoken.get_encoding(name).encode for name in ("cl100k_base", "o200k_base")}
    encoders["Qwen2.5-Coder"] = lambda text: qwen.encode(text, add_special_tokens=False).ids
    rows = []
    for name, (branch, compact) in CASES.items():
        rows.append({"name": name, "branch": branch, "compact": compact,
                     "bytes": {"branch": len(branch.encode()), "compact": len(compact.encode())},
                     "tokens": {encoder: {"branch": len(encode(branch)), "compact": len(encode(compact))}
                                for encoder, encode in encoders.items()}})
    result = {"schema": 1, "tool": "scripts/measure_boolean_logic_tokens.py",
              "versions": {name: importlib.metadata.version(name) for name in ("tiktoken", "tokenizers")},
              "qwen_revision": QWEN_REVISION, "qwen_sha256": QWEN_SHA256,
              "cases": rows}
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(rows, indent=2))


if __name__ == "__main__":
    main()
