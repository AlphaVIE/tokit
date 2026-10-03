"""Measure experimental Tokit JSON parse/render compilation and execution.

These are local wall-clock observations, not cross-language performance claims.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ENTRY = ROOT / "examples" / "json" / "bench_main.tok"
MODULE = ENTRY.with_name("json.tok")
RECORD = '{"id":17,"name":"Grüße","enabled":true,"score":-12.50e+2,"tags":["a","b",null]}'
NESTED = json.dumps(
    {"events": [{"id": i, "ok": i % 2 == 0, "notes": ["plain", "line\nend"]}
                for i in range(12)]},
    ensure_ascii=False, separators=(",", ":"),
)
ESCAPE_VALUE = {"text": "quote \" slash / backslash \\ tab\t emoji 😀" * 12,
                "control": "\u0000\n\r\t"}
FIXTURES = {
    "record": (RECORD, RECORD),
    "nested": (NESTED, NESTED),
    "escapes": (
        json.dumps(ESCAPE_VALUE, ensure_ascii=True, separators=(",", ":")),
        json.dumps(ESCAPE_VALUE, ensure_ascii=False, separators=(",", ":")),
    ),
}


def execute(command: list[str], timeout: int = 120) -> tuple[str, int]:
    start = time.perf_counter_ns()
    result = subprocess.run(
        command, cwd=ROOT, capture_output=True, text=True,
        timeout=timeout, check=False, encoding="utf-8",
    )
    elapsed = time.perf_counter_ns() - start
    if result.returncode:
        raise RuntimeError(
            f"{command[0]} exited {result.returncode}:\n"
            f"{result.stderr[-6000:]}\n{result.stdout[-2000:]}"
        )
    return result.stdout.strip(), elapsed


def summary(samples: list[int]) -> dict[str, int | list[int]]:
    return {"samples_ns": samples, "median_ns": int(statistics.median(samples))}


def main() -> None:
    global ENTRY, MODULE
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--entry", type=Path, default=ENTRY,
                        help="benchmark entry; its sibling json.tok is the module under test")
    parser.add_argument("--iterations", type=int, default=20)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--build-samples", type=int, default=3)
    parser.add_argument("--rust-toolchain", help="rustup toolchain override for native builds")
    parser.add_argument("--tok-binary", type=Path,
                        default=ROOT / "target" / "debug" / ("tok.exe" if os.name == "nt" else "tok"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    ENTRY = args.entry.resolve()
    MODULE = ENTRY.with_name("json.tok")
    if not MODULE.is_file():
        parser.error(f"missing JSON module: {MODULE}")
    if args.iterations < 1 or args.iterations > 10_000:
        parser.error("iterations must be between 1 and 10000")
    if args.warmups < 0 or args.samples < 1 or args.build_samples < 1:
        parser.error("warmups must be nonnegative; samples and build-samples must be positive")
    if not args.tok_binary.is_file() or not shutil.which("rustc"):
        parser.error("build tok first and make rustc available on PATH")
    if args.rust_toolchain:
        os.environ["RUSTUP_TOOLCHAIN"] = args.rust_toolchain

    rust_version, _ = execute(["rustc", "-vV"])
    tok = str(args.tok_binary.resolve())
    with tempfile.TemporaryDirectory(prefix="tokit-json-bench-") as directory:
        binary = Path(directory) / ("json-bench.exe" if os.name == "nt" else "json-bench")
        build = [tok, "build", str(ENTRY), "-o", str(binary)]
        build_times = [execute(build)[1] for _ in range(args.build_samples)]
        workloads = {}
        for name, (document, normalized) in FIXTURES.items():
            expected = f"Some({len(normalized.encode('utf-8')) * args.iterations})"
            commands = {
                "interpreter": [tok, "run", str(ENTRY), "--", document, str(args.iterations)],
                "native": [str(binary), "--", document, str(args.iterations)],
            }
            for _ in range(args.warmups):
                for command in commands.values():
                    actual, _ = execute(command)
                    if actual != expected:
                        raise RuntimeError(f"{name}: expected {expected}, got {actual}")
            times: dict[str, list[int]] = {key: [] for key in commands}
            for sample in range(args.samples):
                names = list(commands)
                if sample % 2:
                    names.reverse()
                for runner in names:
                    actual, elapsed = execute(commands[runner])
                    if actual != expected:
                        raise RuntimeError(f"{name}/{runner}: expected {expected}, got {actual}")
                    times[runner].append(elapsed)
            workloads[name] = {
                "input_bytes": len(document.encode("utf-8")),
                "normalized_bytes": len(normalized.encode("utf-8")),
                "expected_output": expected,
                "run": {runner: summary(values) for runner, values in times.items()},
            }
        report = {
            "host": {"platform": platform.platform(), "machine": platform.machine(),
                     "processor": platform.processor(), "logical_cpus": os.cpu_count(),
                     "python": sys.version.split()[0]},
            "toolchain": {"rustc": rust_version},
            "sources_sha256": {path.relative_to(ROOT).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
                               for path in (ENTRY, MODULE)},
            "iterations": args.iterations,
            "warmups": args.warmups,
            "samples": args.samples,
            "build": summary(build_times),
            "binary_bytes": binary.stat().st_size,
            "workloads": workloads,
        }
    encoded = json.dumps(report, indent=2, ensure_ascii=False) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")


if __name__ == "__main__":
    main()
