#!/usr/bin/env python3
"""Measure local package lock and check costs on synthetic flat graphs."""

import argparse
import json
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


def run(tok, *args):
    result = subprocess.run([tok, *map(str, args)], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"tok {' '.join(map(str, args))}: {result.stderr}")


def measure(tok, args, repetitions):
    samples = []
    for _ in range(repetitions):
        started = time.perf_counter_ns()
        run(tok, *args)
        samples.append((time.perf_counter_ns() - started) / 1_000_000)
    return {"samples_ms": samples, "median_ms": statistics.median(samples)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tok", required=True, type=Path)
    parser.add_argument("--samples", type=int, default=5)
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("--samples must be positive")
    tok = str(args.tok.resolve())
    rows = []
    with tempfile.TemporaryDirectory(prefix="tokit-package-scale-") as temporary:
        root = Path(temporary)
        for count in (0, 1, 8, 32):
            app = root / f"app-{count}"
            app.mkdir()
            entry = app / "main.tok"
            entry.write_text("main()->I{42}\n", encoding="utf-8")
            (app / "tok.toml").write_text("[dependencies]\n", encoding="utf-8")
            for index in range(count):
                package = root / f"source-{count}-{index}.tok"
                package.write_text(f"pub value()->I{{{index}}}\n", encoding="utf-8")
                run(tok, "add", entry, f"p{index:02}", f"../{package.name}")
            if count:
                imports = "".join(f'import p{index:02}="pkg:p{index:02}";\n' for index in range(count))
                expression = "+".join(f"p{index:02}::value()" for index in range(count))
                entry.write_text(f"{imports}main()->I{{{expression}}}\n", encoding="utf-8")
            run(tok, "lock", entry)
            before = (app / "tok.lock").read_bytes()
            lock = measure(tok, ("lock", entry), args.samples)
            assert before == (app / "tok.lock").read_bytes(), "lock output changed"
            checked = measure(tok, ("check", entry), args.samples)
            rows.append({"direct_packages": count, "lock": lock, "check": checked})
    print(json.dumps({"host": platform.platform(), "samples": args.samples, "rows": rows}, indent=2))


if __name__ == "__main__":
    main()
