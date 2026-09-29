"""Compile and run the bounded task-square contract across available toolchains."""

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
FIXTURE = ROOT / "research" / "baselines" / "task-square"


def command(args: list[str]) -> str:
    result = subprocess.run(args, cwd=ROOT, capture_output=True, text=True, timeout=120, check=False)
    if result.returncode:
        raise RuntimeError(f"{' '.join(args)} failed ({result.returncode}):\n{result.stderr}\n{result.stdout}")
    return result.stdout.strip()


def executable(name: str, directory: Path) -> str:
    return str(directory / (name + (".exe" if os.name == "nt" else "")))


def literal(value: int) -> str:
    return str(value) if value >= 0 else f"(0-{abs(value)})"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--require-all", action="store_true")
    parser.add_argument("--rust-toolchain", help="rustup toolchain override")
    args = parser.parse_args()

    cases = json.loads((FIXTURE / "contract.json").read_text(encoding="utf-8"))["cases"]
    tools = {name: shutil.which(name) for name in
             ("cargo", "rustc", "go", "gcc", "g++", "javac", "java", "node")}
    skipped: list[str] = []
    tested: list[str] = []
    with tempfile.TemporaryDirectory(prefix="tokit-task-square-") as raw_directory:
        directory = Path(raw_directory)
        rust_prefix = [f"+{args.rust_toolchain}"] if args.rust_toolchain else []
        runners: dict[str, list[str]] = {"Python": [sys.executable, str(FIXTURE / "python.py")]}
        builds = [
            ("Rust", ("rustc",), [tools["rustc"] or "rustc", *rust_prefix,
             str(FIXTURE / "rust.rs"), "-O", "-o", executable("task-rust", directory)]),
            ("Go", ("go",), [tools["go"] or "go", "build", "-o", executable("task-go", directory),
             str(FIXTURE / "go.go")]),
            ("C", ("gcc",), [tools["gcc"] or "gcc", "-std=c11", "-O2", "-pthread", "-Wall", "-Wextra", "-Werror",
             str(FIXTURE / "c.c"), "-o", executable("task-c", directory)]),
            ("C++", ("g++",), [tools["g++"] or "g++", "-std=c++17", "-O2", "-pthread", "-Wall", "-Wextra", "-Werror",
             str(FIXTURE / "cpp.cpp"), "-o", executable("task-cpp", directory)]),
            ("Java", ("javac", "java"), [tools["javac"] or "javac", "-d", str(directory),
             str(FIXTURE / "Main.java")]),
        ]
        for language, requirements, build in builds:
            if not all(tools[name] for name in requirements):
                skipped.append(language)
                continue
            command(build)
            runners[language] = ([tools["java"] or "java", "-cp", str(directory), "Main"]
                                 if language == "Java" else [executable("task-" + language.lower().replace("+", "p"), directory)])

        if tools["node"]:
            runners["TypeScript"] = [tools["node"], "--experimental-strip-types", str(FIXTURE / "typescript.ts")]
        else:
            skipped.append("TypeScript")

        if tools["cargo"] and tools["rustc"]:
            command([tools["cargo"], *rust_prefix, "build", "--quiet", "-p", "tokit-compiler", "--bin", "tok"])
            runners["Tokit"] = [executable("tok", ROOT / "target" / "debug"), "run"]
        else:
            skipped.append("Tokit")

        core = (FIXTURE / "tokit.tok").read_text(encoding="utf-8")
        for case in cases:
            for language, runner in runners.items():
                if language == "Tokit":
                    source_path = directory / "case.tok"
                    source_path.write_text(
                        core + f"\nfn main()->Result<i32,TaskError>{{square_async({literal(case['input'])})}}\n",
                        encoding="utf-8",
                    )
                    output = command([*runner, str(source_path)])
                else:
                    output = command([*runner, str(case["input"])])
                if output != case["expected"]:
                    raise RuntimeError(
                        f"{language}: {case['name']} returned {output!r}, expected {case['expected']!r}"
                    )
        tested = sorted(runners)

    if args.require_all and skipped:
        parser.error(f"missing toolchains: {', '.join(skipped)}")
    print(json.dumps({"fixture": "task-square", "cases": len(cases),
                      "tested": tested, "skipped": sorted(skipped)}, indent=2))


if __name__ == "__main__":
    main()
