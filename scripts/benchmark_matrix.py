#!/usr/bin/env python3
"""Measure benchmark snapshots and print a compact terminal matrix.

The script discovers benchmark folders by glob pattern, measures source-token
counts for code files, and times three phases where they exist:

- initialization: discovery, source loading, tokenization, and plan setup
- compile: any language-specific build or transpile step
- runtime: execution of the built artifact or interpreter entrypoint

It is intentionally cross-platform and only depends on the standard library
plus an optional tiktoken installation for source token counts.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Callable
from functools import lru_cache

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PATTERNS = ["benchmarks_*"]
CODE_EXTENSIONS = {
    ".tok": "tokit",
    ".js": "javascript",
    ".ts": "typescript",
    ".go": "go",
    ".cpp": "cpp",
    ".cs": "csharp",
    ".py": "python",
    ".rs": "rust",
}


@dataclasses.dataclass(frozen=True)
class BenchSource:
    folder: Path
    path: Path
    language: str


@dataclasses.dataclass
class Measurement:
    folder: str
    file: str
    language: str
    source_bytes: int
    source_tokens: int
    init_ns: int | None = None
    compile_ns: int | None = None
    runtime_ns: int | None = None
    status: str = "pending"
    note: str = ""


@dataclasses.dataclass(frozen=True)
class TokenEncoder:
    name: str
    encode: Callable[[str], list[int]]


def pick_token_encoder() -> TokenEncoder:
    try:
        import tiktoken
    except ImportError as exc:  # pragma: no cover - only hit when dependency is missing
        token_pattern = re.compile(r"\w+|[^\s\w]", re.UNICODE)

        def fallback_encode(source: str) -> list[int]:
            return [0 for _ in token_pattern.findall(source)]

        return TokenEncoder(name="heuristic", encode=fallback_encode)

    return TokenEncoder(
        name="cl100k_base", encode=tiktoken.get_encoding("cl100k_base").encode
    )


def detect_benchmark_folders(patterns: list[str]) -> list[Path]:
    matches: set[Path] = set()
    for pattern in patterns:
        for candidate in ROOT.glob(pattern):
            if candidate.is_dir() and candidate.name.startswith("benchmarks_"):
                matches.add(candidate)
    return sorted(matches)


def collect_sources(folder: Path) -> list[BenchSource]:
    sources: list[BenchSource] = []
    for path in sorted(folder.iterdir()):
        if path.is_file() and path.suffix in CODE_EXTENSIONS:
            sources.append(
                BenchSource(
                    folder=folder, path=path, language=CODE_EXTENSIONS[path.suffix]
                )
            )
    return sources


def run(command: list[str], *, cwd: Path | None = None) -> tuple[int, str, str, int]:
    start = time.perf_counter_ns()
    result = subprocess.run(
        command, cwd=cwd, capture_output=True, text=True, check=False
    )
    elapsed = time.perf_counter_ns() - start
    return result.returncode, result.stdout, result.stderr, elapsed


def ensure_tool(name: str) -> str:
    path = shutil.which(name)
    if not path:
        raise FileNotFoundError(name)
    return path


def cxx_compiler() -> str:
    for candidate in ("c++", "clang++", "g++"):
        path = shutil.which(candidate)
        if path:
            return path
    raise FileNotFoundError("c++/clang++/g++")


def go_compiler() -> str:
    return ensure_tool("go")


@lru_cache(maxsize=1)
def tok_binary() -> tuple[str, ...]:
    suffix = ".exe" if os.name == "nt" else ""
    candidate = ROOT / "target" / "debug" / f"tok{suffix}"
    if candidate.exists():
        return (str(candidate),)
    cargo = ensure_tool("cargo")
    run(
        [cargo, "build", "-p", "tokit-compiler", "--bin", "tok"],
        cwd=ROOT,
    )
    if not candidate.exists():
        raise FileNotFoundError(candidate)
    return (str(candidate),)


def make_cs_project(temp_dir: Path, source: Path) -> Path:
    project_name = source.stem
    project_file = temp_dir / f"{project_name}.csproj"
    project_file.write_text(
        "\n".join(
            [
                '<Project Sdk="Microsoft.NET.Sdk">',
                "  <PropertyGroup>",
                "    <OutputType>Exe</OutputType>",
                "    <TargetFramework>net8.0</TargetFramework>",
                "    <ImplicitUsings>enable</ImplicitUsings>",
                "    <Nullable>enable</Nullable>",
                "  </PropertyGroup>",
                "  <ItemGroup>",
                f'    <Compile Include="{source.name}" />',
                "  </ItemGroup>",
                "</Project>",
            ]
        ),
        encoding="utf-8",
    )
    return project_file


def compile_and_run(
    source: BenchSource, iterations: int, temp_dir: Path
) -> tuple[int | None, int, int, str]:
    """Return compile ns, runtime ns, exit code, and note."""

    source_path = source.path
    language = source.language
    compile_ns: int | None = None
    runtime_ns = 0
    return_code = 0
    note = ""

    if language == "tokit":
        output = temp_dir / (source_path.stem + (".exe" if os.name == "nt" else ""))
        compile_command = list(tok_binary()) + [
            "build",
            str(source_path),
            "-o",
            str(output),
        ]
        return_code, _, stderr, compile_ns = run(compile_command, cwd=ROOT)
        if return_code:
            return compile_ns, 0, return_code, stderr.strip()
        runtime_command = [str(output), str(iterations)]
    elif language == "javascript":
        runtime_command = [ensure_tool("node"), str(source_path), str(iterations)]
    elif language == "typescript":
        tsc = shutil.which("tsc")
        if tsc is None:
            npx = ensure_tool("npx")
            tsc_command = [npx, "--yes", "-p", "typescript", "tsc"]
        else:
            tsc_command = [tsc]
        compiled = temp_dir / f"{source_path.stem}.js"
        compile_command = [
            *tsc_command,
            "--target",
            "ES2020",
            "--module",
            "commonjs",
            "--outDir",
            str(temp_dir),
            str(source_path),
        ]
        return_code, _, stderr, compile_ns = run(compile_command, cwd=ROOT)
        if return_code:
            return compile_ns, 0, return_code, stderr.strip()
        runtime_command = [ensure_tool("node"), str(compiled), str(iterations)]
    elif language == "go":
        compiler = go_compiler()
        output = temp_dir / (source_path.stem + (".exe" if os.name == "nt" else ""))
        compile_command = [compiler, "build", "-o", str(output), str(source_path)]
        return_code, _, stderr, compile_ns = run(compile_command, cwd=ROOT)
        if return_code:
            return compile_ns, 0, return_code, stderr.strip()
        runtime_command = [str(output), str(iterations)]
    elif language == "cpp":
        compiler = cxx_compiler()
        output = temp_dir / (source_path.stem + (".exe" if os.name == "nt" else ""))
        compile_command = [
            compiler,
            "-std=c++20",
            "-O2",
            str(source_path),
            "-o",
            str(output),
        ]
        return_code, _, stderr, compile_ns = run(compile_command, cwd=ROOT)
        if return_code:
            return compile_ns, 0, return_code, stderr.strip()
        runtime_command = [str(output), str(iterations)]
    elif language == "csharp":
        dotnet = ensure_tool("dotnet")
        project_dir = temp_dir / source_path.stem
        project_dir.mkdir(parents=True, exist_ok=True)
        copied_source = project_dir / source_path.name
        copied_source.write_text(
            source_path.read_text(encoding="utf-8"), encoding="utf-8"
        )
        project_file = make_cs_project(project_dir, copied_source)
        compile_command = [
            dotnet,
            "build",
            str(project_file),
            "-c",
            "Release",
            "-o",
            str(project_dir / "out"),
            "-nologo",
        ]
        return_code, _, stderr, compile_ns = run(compile_command, cwd=ROOT)
        if return_code:
            return compile_ns, 0, return_code, stderr.strip()
        runtime_command = [
            dotnet,
            str(project_dir / "out" / f"{source_path.stem}.dll"),
            str(iterations),
        ]
    elif language == "python":
        runtime_command = [sys.executable, str(source_path), str(iterations)]
    elif language == "rust":
        compiler = ensure_tool("rustc")
        output = temp_dir / (source_path.stem + (".exe" if os.name == "nt" else ""))
        compile_command = [
            compiler,
            "--edition=2024",
            "-C",
            "opt-level=2",
            str(source_path),
            "-o",
            str(output),
        ]
        return_code, _, stderr, compile_ns = run(compile_command, cwd=ROOT)
        if return_code:
            return compile_ns, 0, return_code, stderr.strip()
        runtime_command = [str(output), str(iterations)]
    else:
        raise ValueError(f"unsupported language: {language}")

    return_code, stdout, stderr, runtime_ns = run(runtime_command, cwd=ROOT)
    if return_code:
        return compile_ns, runtime_ns, return_code, stderr.strip() or stdout.strip()
    note = stdout.strip()
    return compile_ns, runtime_ns, return_code, note


def format_ns(value: int | None) -> str:
    if value is None:
        return "n/a"
    return f"{value / 1_000_000:.3f}"


def render_table(rows: list[Measurement]) -> str:
    headers = [
        "folder",
        "file",
        "lang",
        "init_ms",
        "compile_ms",
        "runtime_ms",
        "tokens",
        "bytes",
        "status",
    ]
    data = [
        [
            row.folder,
            row.file,
            row.language,
            format_ns(row.init_ns),
            format_ns(row.compile_ns),
            format_ns(row.runtime_ns),
            str(row.source_tokens),
            str(row.source_bytes),
            row.status,
        ]
        for row in rows
    ]
    widths = [len(column) for column in headers]
    for row in data:
        for index, cell in enumerate(row):
            widths[index] = max(widths[index], len(cell))

    def line(columns: list[str]) -> str:
        return " | ".join(
            cell.ljust(widths[index]) for index, cell in enumerate(columns)
        )

    separator = "-+-".join("-" * width for width in widths)
    lines = [line(headers), separator]
    for row in data:
        lines.append(line(row))
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "patterns",
        nargs="*",
        help="benchmark folder glob patterns relative to the repository root",
    )
    parser.add_argument("--iterations", type=int, default=99_999_999)
    parser.add_argument(
        "--json", action="store_true", help="also print machine-readable JSON"
    )
    args = parser.parse_args()

    patterns = args.patterns or DEFAULT_PATTERNS
    folders = detect_benchmark_folders(patterns)
    if not folders:
        raise SystemExit(f"no benchmark folders matched: {', '.join(patterns)}")
    if args.iterations <= 0:
        raise SystemExit("iterations must be positive")

    token_encoder = pick_token_encoder()
    results: list[Measurement] = []

    with tempfile.TemporaryDirectory(prefix="tokit-benchmark-matrix-") as temp_root:
        temp_root_path = Path(temp_root)
        for folder in folders:
            for source in collect_sources(folder):
                init_start = time.perf_counter_ns()
                source_text = source.path.read_text(encoding="utf-8")
                source_bytes = len(source.path.read_bytes())
                source_tokens = len(token_encoder.encode(source_text))
                init_ns = time.perf_counter_ns() - init_start

                measurement = Measurement(
                    folder=folder.name,
                    file=source.path.name,
                    language=source.language,
                    source_bytes=source_bytes,
                    source_tokens=source_tokens,
                    init_ns=init_ns,
                )

                try:
                    run_root = temp_root_path / folder.name / source.path.stem
                    run_root.mkdir(parents=True, exist_ok=True)
                    compile_ns, runtime_ns, code, note = compile_and_run(
                        source,
                        args.iterations,
                        run_root,
                    )
                    measurement.compile_ns = compile_ns
                    measurement.runtime_ns = runtime_ns
                    measurement.status = "ok" if code == 0 else f"exit:{code}"
                    measurement.note = note
                except FileNotFoundError as exc:
                    measurement.status = "missing-tool"
                    measurement.note = str(exc)
                except (
                    Exception
                ) as exc:  # pragma: no cover - surfaced in terminal on real use
                    measurement.status = "error"
                    measurement.note = str(exc)

                results.append(measurement)

    print(f"tokenizer: {token_encoder.name}")
    print(render_table(results))
    if args.json:
        print(
            json.dumps(
                [dataclasses.asdict(row) for row in results],
                indent=2,
                ensure_ascii=False,
            )
        )


if __name__ == "__main__":
    main()
