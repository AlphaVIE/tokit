"""Measure source size using pinned, locally installed tiktoken encodings.

This is a source-only instrument, not a model-generation benchmark.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def measure(path: Path, encodings: dict[str, object]) -> dict[str, object]:
    data = path.read_bytes()
    source = data.decode("utf-8")
    return {
        "file": path.as_posix(),
        "bytes": len(data),
        "chars": len(source),
        "tokens": {name: len(encoding.encode(source)) for name, encoding in encodings.items()},
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", type=Path, help="A file or directory of *.tok.txt files")
    parser.add_argument(
        "--encoding", action="append", dest="encodings",
        help="tiktoken encoding name; repeat to compare (default: cl100k_base, o200k_base)",
    )
    args = parser.parse_args()

    import importlib.metadata
    import tiktoken

    names = args.encodings or ["cl100k_base", "o200k_base"]
    paths = sorted(args.path.glob("*.tok.txt")) if args.path.is_dir() else [args.path]
    if not paths:
        parser.error("no candidate files found")
    encodings = {name: tiktoken.get_encoding(name) for name in names}
    result = {
        "tool": "scripts/token_cost.py",
        "tiktoken_version": importlib.metadata.version("tiktoken"),
        "encodings": names,
        "measurements": [measure(path, encodings) for path in paths],
    }
    print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
