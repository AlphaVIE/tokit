"""Measure source token savings from experimental Tokit syntax variants."""
from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path
import subprocess
import tempfile

from token_cost import QWEN_REVISION, QWEN_SHA256


def command(tok: Path, *args: str) -> str:
    return subprocess.run([str(tok), *args], check=True, capture_output=True, encoding="utf-8").stdout


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--examples", type=Path, default=Path("examples"))
    parser.add_argument("--tok", type=Path, required=True)
    parser.add_argument("--qwen-tokenizer", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--mode", choices=("all", "functions-only", "types-only"), default="functions-only")
    args = parser.parse_args()
    import tiktoken
    from tokenizers import Tokenizer
    digest = hashlib.sha256(args.qwen_tokenizer.read_bytes()).hexdigest()
    if digest != QWEN_SHA256:
        parser.error(f"Qwen tokenizer hash mismatch: {digest}")
    qwen = Tokenizer.from_file(str(args.qwen_tokenizer))
    encoders = {name: tiktoken.get_encoding(name).encode for name in ("cl100k_base", "o200k_base")}
    encoders["Qwen2.5-Coder"] = lambda text: qwen.encode(text, add_special_tokens=False).ids
    rows = []
    for path in sorted(args.examples.rglob("*.tok")):
        source = path.read_text(encoding="utf-8").replace("\r\n", "\n")
        with tempfile.TemporaryDirectory(prefix="tokit-compact-") as temporary:
            test_path = Path(temporary) / "test.tok"
            test_path.write_bytes(source.encode())
            option = [] if args.mode == "all" else ["--" + args.mode]
            compact = command(args.tok.resolve(), "compact", *option, str(test_path))
            test_path.write_bytes(compact.encode())
            if command(args.tok.resolve(), "compact", *option, str(test_path)) != compact:
                raise ValueError(f"{path}: compact output not idempotent")
            command(args.tok.resolve(), "fmt", str(test_path))
            test_path.write_bytes(source.encode())
            index = json.loads(command(args.tok.resolve(), "ai-patch-index", str(test_path)))
        rows.append({"path": path.as_posix(), "source_sha256": hashlib.sha256(source.encode()).hexdigest(),
                     "compact_sha256": hashlib.sha256(compact.encode()).hexdigest(),
                     "functions": len(index["functions"]),
                     "bytes": {"source": len(source.encode()), "compact": len(compact.encode())},
                     "tokens": {name: {"source": len(encode(source)), "compact": len(encode(compact))}
                                for name, encode in encoders.items()}})
    result = {"schema": 1, "tool": "scripts/measure_compact_functions.py", "mode": args.mode,
              "compiler_sha256": hashlib.sha256(args.tok.read_bytes()).hexdigest(),
              "versions": {name: importlib.metadata.version(name) for name in ("tiktoken", "tokenizers")},
              "qwen_revision": QWEN_REVISION, "qwen_sha256": QWEN_SHA256,
              "summary": {"files": len(rows), "functions": sum(row["functions"] for row in rows),
                          "bytes": {key: sum(row["bytes"][key] for row in rows) for key in ("source", "compact")},
                          "tokens": {name: {key: sum(row["tokens"][name][key] for row in rows)
                                            for key in ("source", "compact")}
                                     for name in encoders}},
              "files": rows}
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result["summary"], indent=2))


if __name__ == "__main__":
    main()
