"""Compile and run the sum-positive contract across available language toolchains."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "research" / "baselines" / "sum-positive"


def command(args: list[str], cwd: Path = ROOT) -> str:
    result = subprocess.run(args, cwd=cwd, capture_output=True, text=True, timeout=120, check=False)
    if result.returncode:
        raise RuntimeError(f"{' '.join(args)} failed ({result.returncode}):\n{result.stderr}\n{result.stdout}")
    return result.stdout.strip()


def executable(name: str, directory: Path) -> str:
    return str(directory / (name + (".exe" if os.name == "nt" else "")))


def tokit_literal(value: int) -> str:
    return str(value) if value >= 0 else f"(0-{abs(value)})"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--require-all", action="store_true", help="fail if any toolchain is missing")
    parser.add_argument("--rust-toolchain", help="rustup toolchain override, e.g. stable-x86_64-pc-windows-gnu")
    args = parser.parse_args()

    cases = json.loads((FIXTURE / "contract.json").read_text(encoding="utf-8"))["cases"]
    tools = {name: shutil.which(name) for name in
             ("cargo", "rustc", "go", "gcc", "g++", "javac", "java", "node")}
    skipped: list[str] = []
    tested: list[str] = []
    with tempfile.TemporaryDirectory(prefix="tokit-baselines-") as raw_directory:
        directory = Path(raw_directory)
        rust_prefix = [f"+{args.rust_toolchain}"] if args.rust_toolchain else []
        runners: dict[str, list[str]] = {"Python": [sys.executable, str(FIXTURE / "python.py")]}

        build_specs = [
            ("Rust", ("rustc",), [tools["rustc"] or "rustc", *rust_prefix,
             str(FIXTURE / "rust.rs"), "-O", "-o", executable("sum-rust", directory)]),
            ("Go", ("go",), [tools["go"] or "go", "build", "-o", executable("sum-go", directory),
             str(FIXTURE / "go.go")]),
            ("C", ("gcc",), [tools["gcc"] or "gcc", "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
             str(FIXTURE / "c.c"), "-o", executable("sum-c", directory)]),
            ("C++", ("g++",), [tools["g++"] or "g++", "-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror",
             str(FIXTURE / "cpp.cpp"), "-o", executable("sum-cpp", directory)]),
            ("Java", ("javac", "java"), [tools["javac"] or "javac", "-d", str(directory),
             str(FIXTURE / "Main.java")]),
        ]
        for language, requirements, build in build_specs:
            if not all(tools[name] for name in requirements):
                skipped.append(language)
                continue
            command(build)
            runners[language] = ([tools["java"] or "java", "-cp", str(directory), "Main"]
                                 if language == "Java" else [executable("sum-" + language.lower().replace("+", "p"), directory)])

        if tools["node"]:
            runners["TypeScript"] = [tools["node"], "--experimental-strip-types", str(FIXTURE / "typescript.ts")]
        else:
            skipped.append("TypeScript")

        if tools["cargo"] and tools["rustc"]:
            command([tools["cargo"], *rust_prefix, "build", "--quiet", "-p", "tokit-compiler", "--bin", "tok"])
            runners["Tokit"] = [executable("tok", ROOT / "target" / "debug"), "run"]
        else:
            skipped.append("Tokit")

        for language, runner in runners.items():
            for case in cases:
                values = case["input"]
                if language == "Tokit":
                    source = (FIXTURE / "tokit.tok").read_text(encoding="utf-8")
                    literal = ",".join(tokit_literal(value) for value in values)
                    path = directory / "case.tok"
                    path.write_text(source + f"\nfn main()->i32{{sum_positive([{literal}])}}\n", encoding="utf-8")
                    output = command([*runner, str(path)])
                else:
                    output = command([*runner, *(str(value) for value in values)])
                if output != str(case["expected"]):
                    raise RuntimeError(f"{language}: {values} returned {output!r}, expected {case['expected']}")
            tested.append(language)

    if args.require_all and skipped:
        parser.error(f"missing toolchains: {', '.join(skipped)}")
    print(json.dumps({"fixture": "sum-positive", "cases": len(cases), "tested": sorted(tested),
                      "skipped": sorted(skipped)}, indent=2))


if __name__ == "__main__":
    main()
