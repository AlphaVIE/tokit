"""Compare checked whole-process record-field reads before and after a change."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import platform
import statistics
import subprocess
from time import perf_counter_ns


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def timed(command: list[str], expected: str) -> float:
    start = perf_counter_ns()
    run = subprocess.run(command, capture_output=True, text=True, encoding="utf-8")
    elapsed = (perf_counter_ns() - start) / 1_000_000
    if run.returncode != 0 or run.stdout.strip() != expected:
        raise RuntimeError(f"wrong benchmark result: {command!r}: {run.returncode} {run.stdout!r} {run.stderr!r}")
    return elapsed


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--before-tok", type=Path, required=True)
    parser.add_argument("--after-tok", type=Path, required=True)
    parser.add_argument("--before-native", type=Path, required=True)
    parser.add_argument("--after-native", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--iterations", type=int, default=2000)
    parser.add_argument("--sizes", type=int, nargs="+", default=[4096, 16384])
    parser.add_argument("--pairs", type=int, default=4)
    args = parser.parse_args()
    if args.iterations < 0 or any(size < 0 for size in args.sizes) or args.pairs < 1:
        parser.error("iterations and sizes must be nonnegative; pairs must be positive")
    binaries = {
        "interpreter": {"before": args.before_tok, "after": args.after_tok},
        "native": {"before": args.before_native, "after": args.after_native},
    }
    rows = []
    for mode, versions in binaries.items():
        for size in args.sizes:
            expected = f"Ok({7 * args.iterations})"
            commands = {
                label: ([str(binary.resolve()), "run", str(args.fixture.resolve()), "--", str(args.iterations), str(size)]
                        if mode == "interpreter" else [str(binary.resolve()), str(args.iterations), str(size)])
                for label, binary in versions.items()
            }
            for command in commands.values():
                timed(command, expected)
            samples = {"before": [], "after": []}
            for _ in range(args.pairs):
                for label in ("before", "after", "after", "before"):
                    samples[label].append(timed(commands[label], expected))
            rows.append({"mode": mode, "elements": size, "iterations": args.iterations,
                         "expected": expected, "samples_ms": samples,
                         "median_ms": {label: statistics.median(values) for label, values in samples.items()}})
    result = {
        "schema": 1,
        "fixture": args.fixture.as_posix(),
        "fixture_sha256": digest(args.fixture),
        "platform": platform.platform(),
        "rustc": subprocess.run(["rustc", "--version"], capture_output=True, text=True, check=True).stdout.strip(),
        "binaries_sha256": {mode: {label: digest(binary) for label, binary in versions.items()}
                            for mode, versions in binaries.items()},
        "warmups_per_variant": 1,
        "pairs": args.pairs,
        "rows": rows,
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps([{"mode": row["mode"], "elements": row["elements"], "median_ms": row["median_ms"]}
                      for row in rows], indent=2))


if __name__ == "__main__":
    main()
