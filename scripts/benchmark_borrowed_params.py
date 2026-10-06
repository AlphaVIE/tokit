"""Compare native runtimes of two Tokit CLIs on parameter-passing workloads.

Every run of a workload must print the same output with both compilers.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before", type=Path, required=True, help="baseline tok executable")
    parser.add_argument("--after", type=Path, required=True, help="candidate tok executable")
    parser.add_argument("--work", type=Path, required=True, help="directory for built binaries")
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--output", type=Path)
    options = parser.parse_args()
    options.work.mkdir(parents=True, exist_ok=True)
    document = json.dumps(
        {"items": [{"id": i, "name": f"item{i}", "tags": ["a", "b"], "ok": True} for i in range(60)]},
        separators=(",", ":"),
    )
    cases = {
        "array_arg": (ROOT / "benchmarks/borrowed_params/array_arg.tok", []),
        "record_arg": (ROOT / "benchmarks/borrowed_params/record_arg.tok", []),
        "json_roundtrip": (ROOT / "examples/json/bench_main.tok", [document, "300", document]),
        "json_render": (ROOT / "examples/json/render_bench_main.tok", [document, "3000", document]),
    }
    compilers = {"before": options.before, "after": options.after}
    report = {"compilers": {k: sha256(v) for k, v in compilers.items()}, "cases": {}}
    for name, (source, args) in cases.items():
        entry = {"source_sha256": sha256(source), "variants": {}}
        for label, tok in compilers.items():
            exe = options.work / f"{name}_{label}"
            subprocess.run([str(tok), "build", str(source), "-o", str(exe)], check=True, capture_output=True)
            exe = next(options.work.glob(f"{name}_{label}*"))
            samples, outputs = [], set()
            for _ in range(options.samples):
                start = time.perf_counter()
                run = subprocess.run([str(exe), "--", *args], check=True, capture_output=True, text=True)
                samples.append((time.perf_counter() - start) * 1000)
                outputs.add(run.stdout.strip())
            if len(outputs) != 1:
                raise SystemExit(f"{name}/{label}: unstable output {outputs}")
            entry["variants"][label] = {"output": outputs.pop(), "samples_ms": samples, "median_ms": statistics.median(samples)}
        before, after = entry["variants"]["before"], entry["variants"]["after"]
        if before["output"] != after["output"]:
            raise SystemExit(f"{name}: outputs differ")
        entry["ratio"] = before["median_ms"] / after["median_ms"]
        report["cases"][name] = entry
        print(f"{name:15s} {before['median_ms']:9.1f} ms -> {after['median_ms']:9.1f} ms ({entry['ratio']:.2f}x)")
    if options.output:
        options.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
