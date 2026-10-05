"""Validate real function patches and count their complete text payloads."""
from __future__ import annotations

import argparse
import difflib
import hashlib
import importlib.metadata
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

from token_cost import QWEN_REVISION, QWEN_SHA256


def run(tok: Path, *args: str) -> str:
    return subprocess.run([str(tok), *args], capture_output=True, text=True,
                          encoding="utf-8", check=True).stdout


def compact(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def validate_case(tok: Path, root: Path, case: dict, targeted_index: bool = False) -> dict[str, str]:
    source_path = root / case["source"]
    before = source_path.read_bytes().decode("utf-8")
    if hashlib.sha256(before.encode()).hexdigest() != case["sha256"]:
        raise ValueError(f"{case['name']}: source snapshot hash mismatch")
    entry = root / case["entry"]
    args = case["args"]
    if run(tok, "run", str(entry), "--", *args).strip() != case["before"]:
        raise ValueError(f"{case['name']}: original output mismatch")
    index = run(tok, "ai-patch-index", str(source_path)).strip()
    records = json.loads(index)["functions"]
    selected, = [item for item in records if item["name"] == case["function"]]
    targeted = {}
    if targeted_index:
        response = run(tok, "ai-patch-index", str(source_path), case["function"]).strip()
        if json.loads(response) != {"version": 1, "functions": [selected]}:
            raise ValueError("targeted index differs from full index")
        targeted["targeted_index_response"] = response
    start, end = selected["span"]
    data = before.encode("utf-8")
    if hashlib.sha256(data[start:end]).hexdigest() != selected["sha256"]:
        raise ValueError("index hash does not match its source span")
    replacement = (root / case["replacement"]).read_text(encoding="utf-8")
    after = (data[:start] + replacement.strip().encode() + data[end:]).decode()
    request = compact({"version": 1, "target": source_path.relative_to(entry.parent).as_posix(),
                       "edits": [{"function": case["function"],
                                  "expected_sha256": selected["sha256"],
                                  "replacement": replacement}]})
    with tempfile.TemporaryDirectory(prefix="tokit-patch-cost-") as directory:
        scratch = Path(directory)
        shutil.copytree(root, scratch, dirs_exist_ok=True)
        request_path = scratch / "request.json"
        request_path.write_bytes(request.encode())
        scratch_entry = scratch / case["entry"]
        scratch_source = scratch / case["source"]
        preview = run(tok, "ai-patch", str(scratch_entry), str(request_path))
        if preview != after + "\n" or scratch_source.read_bytes() != data:
            raise ValueError("preview differs or modified its source")
        status = json.loads(run(tok, "ai-patch", str(scratch_entry), str(request_path), "--write"))
        if not status["ok"] or scratch_source.read_bytes() != after.encode():
            raise ValueError("written patch differs from independent byte splice")
        if run(tok, "run", str(scratch_entry), "--", *args).strip() != case["after"]:
            raise ValueError(f"{case['name']}: patched output mismatch")
    return {**targeted, "full_source": after, "patch_request": request, "full_index": index,
            "selected_index_entry": compact(selected),
            "unified_diff": "".join(difflib.unified_diff(
                before.splitlines(keepends=True), after.splitlines(keepends=True),
                fromfile="a/source.tok", tofile="b/source.tok", n=3))}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dataset", type=Path)
    parser.add_argument("--tok", type=Path, required=True)
    parser.add_argument("--qwen-tokenizer", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--targeted-index", action="store_true")
    args = parser.parse_args()
    contract = json.loads((args.dataset / "contract.json").read_text(encoding="utf-8"))
    if contract["version"] != 1:
        parser.error("unsupported contract")
    validated = [(case, validate_case(args.tok.resolve(), args.dataset.resolve(), case, args.targeted_index))
                 for case in contract["cases"]]
    if args.verify_only:
        print(f"Verified {len(validated)} function patch cases")
        return
    if not args.qwen_tokenizer or not args.output:
        parser.error("token measurement requires --qwen-tokenizer and --output")
    import tiktoken
    from tokenizers import Tokenizer
    if hashlib.sha256(args.qwen_tokenizer.read_bytes()).hexdigest() != QWEN_SHA256:
        parser.error("Qwen tokenizer digest mismatch")
    tokenizer = Tokenizer.from_file(str(args.qwen_tokenizer))
    encoders = {name: tiktoken.get_encoding(name).encode for name in ("cl100k_base", "o200k_base")}
    encoders["Qwen2.5-Coder"] = lambda text: tokenizer.encode(text, add_special_tokens=False).ids
    rows = []
    for case, payloads in validated:
        rows.append({"case": case["name"], "source_sha256": case["sha256"],
                     "payloads": {name: {"sha256": hashlib.sha256(text.encode()).hexdigest(),
                                         "bytes": len(text.encode()),
                                         "tokens": {label: len(encode(text)) for label, encode in encoders.items()}}
                                  for name, text in payloads.items()}})
    result = {"schema": 1, "tool": "scripts/benchmark_function_patches.py",
              "compiler_sha256": hashlib.sha256(args.tok.read_bytes()).hexdigest(),
              "versions": {name: importlib.metadata.version(name) for name in ("tiktoken", "tokenizers")},
              "qwen_revision": QWEN_REVISION, "qwen_sha256": QWEN_SHA256,
              "cases": rows}
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
