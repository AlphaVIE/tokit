"""Fetch the pinned Qwen tokenizer into an ignored local cache, verifying SHA-256."""

from __future__ import annotations

import argparse
import hashlib
import urllib.request
from pathlib import Path

from token_cost import QWEN_REVISION, QWEN_SHA256


URL = (
    "https://huggingface.co/Qwen/Qwen2.5-Coder-0.5B-Instruct/"
    f"resolve/{QWEN_REVISION}/tokenizer.json"
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", type=Path,
        default=Path("research/.cache/qwen2.5-coder-tokenizer.json"),
    )
    args = parser.parse_args()
    if args.output.exists():
        data = args.output.read_bytes()
    else:
        with urllib.request.urlopen(URL, timeout=60) as response:
            data = response.read()
    digest = hashlib.sha256(data).hexdigest()
    if digest != QWEN_SHA256:
        parser.error(f"SHA-256 mismatch: {digest}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(data)
    print(f"{args.output}: {len(data)} bytes, sha256={digest}")


if __name__ == "__main__":
    main()
