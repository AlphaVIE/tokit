"""Measure the Tokit native bootstrap against an equivalent Rust workload."""

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
WORKLOAD = ROOT / "benchmarks" / "native"


def execute(command: list[str], environment: dict[str, str]) -> tuple[str, int]:
    start = time.perf_counter_ns()
    result = subprocess.run(
        command, cwd=ROOT, env=environment, capture_output=True, text=True,
        timeout=120, check=False,
    )
    elapsed = time.perf_counter_ns() - start
    if result.returncode:
        raise RuntimeError(
            f"{command[0]} exited {result.returncode}:\n{result.stderr}\n{result.stdout}"
        )
    return result.stdout.strip(), elapsed


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def summary(samples: list[int]) -> dict[str, int | list[int]]:
    return {
        "samples_ns": samples,
        "median_ns": int(statistics.median(samples)),
        "min_ns": min(samples),
        "max_ns": max(samples),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workload", choices=("cycle-sum", "array-cycle"), default="cycle-sum")
    parser.add_argument("--iterations", type=int, default=99_999_999)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--build-samples", type=int, default=3)
    parser.add_argument("--rust-toolchain", help="rustup toolchain override")
    parser.add_argument("--output", type=Path, help="also save JSON at this path")
    args = parser.parse_args()
    if not 0 < args.iterations <= 100_000_000 or args.iterations % 3:
        parser.error("iterations must be a positive multiple of 3, at most 100000000")
    if args.warmups < 0 or args.samples < 1 or args.build_samples < 1:
        parser.error("warmups must be nonnegative; samples and build-samples must be positive")
    if not shutil.which("cargo") or not shutil.which("rustc"):
        parser.error("cargo and rustc must be on PATH")

    environment = os.environ.copy()
    if args.rust_toolchain:
        environment["RUSTUP_TOOLCHAIN"] = args.rust_toolchain
    rust_version, _ = execute(["rustc", "-vV"], environment)
    cargo_version, _ = execute(["cargo", "--version"], environment)
    _, preparation_ns = execute(
        ["cargo", "build", "--quiet", "-p", "tokit-compiler", "--bin", "tok"],
        environment,
    )
    suffix = ".exe" if os.name == "nt" else ""
    tok = ROOT / "target" / "debug" / f"tok{suffix}"
    stem = args.workload.replace("-", "_")
    source_tok = WORKLOAD / f"{stem}.tok"
    source_rust = WORKLOAD / f"{stem}.rs"
    expected = f"Ok({2 * args.iterations})"

    with tempfile.TemporaryDirectory(prefix="tokit-native-bench-") as temporary:
        directory = Path(temporary)
        outputs = {name: directory / f"{args.workload}-{name}{suffix}" for name in ("Tokit", "Rust")}
        build_commands = {
            "Tokit": [str(tok), "build", str(source_tok), "-o", str(outputs["Tokit"])],
            "Rust": ["rustc", "--edition=2024", "-C", "opt-level=2", str(source_rust),
                     "-o", str(outputs["Rust"])],
        }
        build_samples: dict[str, list[int]] = {name: [] for name in outputs}
        for _ in range(args.build_samples):
            for name, command in build_commands.items():
                _, elapsed = execute(command, environment)
                build_samples[name].append(elapsed)

        commands = {name: [str(path), str(args.iterations)] for name, path in outputs.items()}
        for name, command in commands.items():
            actual, _ = execute(command, environment)
            if actual != expected:
                raise RuntimeError(f"{name} returned {actual!r}, expected {expected!r}")
        for _ in range(args.warmups):
            for command in commands.values():
                actual, _ = execute(command, environment)
                if actual != expected:
                    raise RuntimeError(f"warmup returned {actual!r}, expected {expected!r}")
        run_samples: dict[str, list[int]] = {name: [] for name in outputs}
        for round_number in range(args.samples):
            names = list(commands)
            if round_number % 2:
                names.reverse()
            for name in names:
                actual, elapsed = execute(commands[name], environment)
                if actual != expected:
                    raise RuntimeError(f"{name} returned {actual!r}, expected {expected!r}")
                run_samples[name].append(elapsed)

        report = {
            "workload": args.workload,
            "iterations": args.iterations,
            "expected_output": expected,
            "host": {"platform": platform.platform(), "machine": platform.machine(),
                     "processor": platform.processor(), "logical_cpus": os.cpu_count(),
                     "python": sys.version.split()[0]},
            "toolchain": {"rustc": rust_version, "cargo": cargo_version,
                          "tokit_bootstrap": "rustc -C opt-level=2"},
            "source_sha256": {"Tokit": digest(source_tok), "Rust": digest(source_rust)},
            "preparation_ns": preparation_ns,
            "warmups": args.warmups,
            "results": {
                name: {"build": summary(build_samples[name]),
                       "run": summary(run_samples[name]),
                       "binary_bytes": outputs[name].stat().st_size}
                for name in outputs
            },
        }
    encoded = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")


if __name__ == "__main__":
    main()
