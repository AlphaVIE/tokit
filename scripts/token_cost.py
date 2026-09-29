"""Measure source size using pinned, locally installed tiktoken encodings.

This is a source-only instrument, not a model-generation benchmark.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
from collections.abc import Callable, Sized
from pathlib import Path


QWEN_REVISION = "ea3f2471cf1b1f0db85067f1ef93848e38e88c25"
QWEN_SHA256 = "c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539"


def measure(path: Path, encodings: dict[str, Callable[[str], Sized]], region: str | None = None) -> dict[str, object]:
    data = path.read_bytes()
    source = data.decode("utf-8")
    if region == "core":
        lines = source.splitlines(keepends=True)
        starts = [i for i, line in enumerate(lines) if "BENCH_START" in line]
        ends = [i for i, line in enumerate(lines) if "BENCH_END" in line]
        if len(starts) != 1 or len(ends) != 1 or starts[0] >= ends[0]:
            raise ValueError(f"{path}: expected one ordered BENCH_START/BENCH_END pair")
        source = "".join(lines[starts[0] + 1:ends[0]])
        data = source.encode("utf-8")
    return {
        "file": path.as_posix(),
        "region": region or "whole",
        "bytes": len(data),
        "chars": len(source),
        "tokens": {name: len(encode(source)) for name, encode in encodings.items()},
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", type=Path, help="A file or directory of *.tok.txt files")
    parser.add_argument("--region", choices=["core"], help="count only content between BENCH markers")
    parser.add_argument(
        "--encoding", action="append", dest="encodings",
        help="tiktoken encoding name; repeat to compare (default: cl100k_base, o200k_base)",
    )
    parser.add_argument(
        "--qwen-tokenizer", type=Path, metavar="TOKENIZER_JSON",
        help="verified local Qwen2.5-Coder-0.5B-Instruct tokenizer.json",
    )
    args = parser.parse_args()

    import tiktoken

    names = args.encodings or ["cl100k_base", "o200k_base"]
    paths = sorted(args.path.rglob("*.tok.txt")) if args.path.is_dir() else [args.path]
    if not paths:
        parser.error("no candidate files found")
    encodings = {name: tiktoken.get_encoding(name).encode for name in names}
    artifacts: dict[str, object] = {}
    if args.qwen_tokenizer:
        from tokenizers import Tokenizer

        digest = hashlib.sha256(args.qwen_tokenizer.read_bytes()).hexdigest()
        if digest != QWEN_SHA256:
            parser.error(f"Qwen tokenizer SHA-256 mismatch: {digest}")
        tokenizer = Tokenizer.from_file(str(args.qwen_tokenizer))
        label = "Qwen2.5-Coder-0.5B-Instruct"
        encodings[label] = lambda source: tokenizer.encode(source, add_special_tokens=False).ids
        artifacts[label] = {
            "repository": "Qwen/Qwen2.5-Coder-0.5B-Instruct",
            "revision": QWEN_REVISION,
            "sha256": digest,
            "tokenizers_version": importlib.metadata.version("tokenizers"),
        }
    result = {
        "tool": "scripts/token_cost.py",
        "tiktoken_version": importlib.metadata.version("tiktoken"),
        "encodings": list(encodings),
        "artifacts": artifacts,
        "measurements": [measure(path, encodings, args.region) for path in paths],
    }
    print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
