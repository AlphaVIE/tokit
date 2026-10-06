"""Evaluate a model on a Tokit dataset split with the compiler as the judge.

For every sample the model receives the system and user messages, the first
```tokit block of its answer is extracted, and the program is checked with
`tok check --json`. Generation samples must then print exactly the expected
output under `tok run`; repair samples must check and equal the reference
after canonicalization. Optionally, failed attempts get the compiler
diagnostic back for further repair rounds (`--repair-rounds`), which models
the recommended agent loop.

Providers:
  openai     any OpenAI-compatible chat endpoint (OpenAI, vLLM, llama.cpp, ...)
  anthropic  the Anthropic Messages API
  oracle     returns each reference answer; a self-test of the judge
Keys come from OPENAI_API_KEY / ANTHROPIC_API_KEY.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import tempfile
import time
import urllib.request
from pathlib import Path

CODE = re.compile(r"```(?:tokit|tok)?\s*\n(.*?)```", re.DOTALL)


def post(url: str, headers: dict, payload: dict) -> dict:
    request = urllib.request.Request(url, data=json.dumps(payload).encode(), headers=headers, method="POST")
    for attempt in range(5):
        try:
            with urllib.request.urlopen(request, timeout=300) as response:
                return json.loads(response.read())
        except Exception:  # noqa: BLE001 - network retries
            if attempt == 4:
                raise
            time.sleep(2 ** attempt)
    raise RuntimeError("unreachable")


def complete(args, messages: list[dict], reference: str) -> tuple[str, dict]:
    if args.provider == "oracle":
        # Answers with the reference: a self-test of the judge (expect 100%).
        return reference, {}
    if args.provider == "anthropic":
        system = "\n\n".join(m["content"] for m in messages if m["role"] == "system")
        rest = [m for m in messages if m["role"] != "system"]
        data = post(
            args.base_url or "https://api.anthropic.com/v1/messages",
            {"x-api-key": os.environ["ANTHROPIC_API_KEY"], "anthropic-version": "2023-06-01",
             "content-type": "application/json"},
            {"model": args.model, "max_tokens": args.max_tokens, "system": system, "messages": rest,
             "temperature": args.temperature},
        )
        text = "".join(block.get("text", "") for block in data["content"])
        return text, data.get("usage", {})
    data = post(
        (args.base_url or "https://api.openai.com/v1") + "/chat/completions",
        {"authorization": f"Bearer {os.environ.get('OPENAI_API_KEY', 'none')}", "content-type": "application/json"},
        {"model": args.model, "messages": messages, "max_tokens": args.max_tokens, "temperature": args.temperature},
    )
    return data["choices"][0]["message"]["content"], data.get("usage", {})


class Judge:
    def __init__(self, tok: Path, work: Path):
        self.tok, self.work, self.counter = tok, work, 0

    def run(self, *args: str, stdin: str | None = None) -> subprocess.CompletedProcess:
        return subprocess.run([str(self.tok), *args], input=stdin, capture_output=True, text=True,
                              encoding="utf-8", timeout=60)

    def file(self, code: str) -> Path:
        self.counter += 1
        path = self.work / f"e{self.counter}.tok"
        path.write_text(code, encoding="utf-8", newline="\n")
        return path

    def diagnostic(self, code: str) -> str | None:
        data = json.loads(self.run("check", "--json", str(self.file(code))).stdout or "{}")
        if data.get("ok"):
            return None
        error = data.get("error", {})
        return f"{error.get('code')} at {error.get('line')}:{error.get('column')}: {error.get('message')}"

    def canonical(self, code: str) -> str:
        path = self.file(code)
        self.run("compact", "--write", str(path))
        self.run("fmt", "--write", str(path))
        return path.read_text(encoding="utf-8").strip()

    def verdict(self, code: str, sample: dict) -> tuple[bool, str | None]:
        meta = sample["meta"]
        if (error := self.diagnostic(code)) is not None:
            return False, error
        if meta["kind"] == "repair":
            reference = CODE.search(sample["messages"][-1]["content"]).group(1).strip()
            return self.canonical(code) == reference, None
        if meta.get("check_only"):
            return True, None
        args = meta.get("args") or []
        result = self.run("run", str(self.file(code)), *(["--", *args] if args else []), stdin=meta.get("stdin"))
        stdout = result.stdout.replace("\r\n", "\n")
        expected = meta["expected"]
        printed = stdout if expected.endswith("\n") else stdout.strip()
        if result.returncode != 0:
            return False, (result.stderr.strip() or "runtime failure")
        return printed == expected, None if printed == expected else f"printed {printed!r}, expected {expected!r}"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--data", type=Path, required=True, help="a split such as test_ood.jsonl")
    parser.add_argument("--tok", type=Path, required=True)
    parser.add_argument("--provider", choices=("openai", "anthropic", "oracle"), default="openai")
    parser.add_argument("--model", default="oracle")
    parser.add_argument("--base-url")
    parser.add_argument("--system-file", type=Path,
                        help="replace each sample's system prompt, e.g. spec/LLM_GUIDE.md for prompting baselines")
    parser.add_argument("--repair-rounds", type=int, default=0)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--max-tokens", type=int, default=2048)
    parser.add_argument("--temperature", type=float, default=0.0)
    parser.add_argument("--out", type=Path, required=True, help="JSONL with one record per sample")
    args = parser.parse_args()

    samples = [json.loads(line) for line in args.data.read_text(encoding="utf-8").splitlines() if line.strip()]
    samples = samples[: args.limit] if args.limit else samples
    system_override = args.system_file.read_text(encoding="utf-8") if args.system_file else None
    totals = {"samples": 0, "passed": 0, "passed_first_try": 0, "compiled_first_try": 0,
              "input_tokens": 0, "output_tokens": 0}
    with tempfile.TemporaryDirectory(prefix="tokit-eval-") as work, args.out.open("w", encoding="utf-8") as out:
        judge = Judge(args.tok.resolve(), Path(work))
        for sample in samples:
            messages = [dict(m) for m in sample["messages"][:-1]]
            if system_override:
                messages = [m for m in messages if m["role"] != "system"]
                messages.insert(0, {"role": "system", "content": system_override})
            attempts = []
            passed = False
            for round_index in range(args.repair_rounds + 1):
                answer, usage = complete(args, messages, sample["messages"][-1]["content"])
                totals["input_tokens"] += usage.get("input_tokens", usage.get("prompt_tokens", 0))
                totals["output_tokens"] += usage.get("output_tokens", usage.get("completion_tokens", 0))
                match = CODE.search(answer)
                code = match.group(1).strip() if match else answer.strip()
                ok, feedback = judge.verdict(code, sample)
                attempts.append({"code": code, "ok": ok, "feedback": feedback})
                if round_index == 0:
                    totals["compiled_first_try"] += judge.diagnostic(code) is None
                    totals["passed_first_try"] += ok
                if ok:
                    passed = True
                    break
                messages += [{"role": "assistant", "content": answer},
                             {"role": "user", "content": f"`tok` reports: {feedback}\nReturn the complete corrected program."}]
            totals["samples"] += 1
            totals["passed"] += passed
            out.write(json.dumps({"family": sample["meta"]["family"], "kind": sample["meta"]["kind"],
                                  "passed": passed, "attempts": attempts}, ensure_ascii=False) + "\n")
            print(f"{totals['passed']}/{totals['samples']} passed", end="\r", flush=True)
    n = max(totals["samples"], 1)
    summary = {**totals, "pass@1": totals["passed_first_try"] / n, "compile@1": totals["compiled_first_try"] / n,
               f"pass@repair{args.repair_rounds}": totals["passed"] / n}
    print()
    print(json.dumps(summary, indent=2))
    args.out.with_suffix(".summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
