#!/usr/bin/env python3
"""Measure versioned benchmark snapshots with verified output and repeated runs.

Compilation is timed once. Runtime includes process startup and reports the
median of successful samples; every warmup and sample must produce the exact
contract result. Source-token counts require tiktoken and are unavailable
when it is not installed.
"""

from __future__ import annotations

import argparse
import dataclasses
import hashlib
import importlib.metadata
import json
import math
import os
import platform
import shutil
import statistics
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
    source_sha256: str
    source_tokens: int | None
    compile_ns: int | None = None
    runtime_ns: int | None = None
    runtime_samples_ns: list[int] = dataclasses.field(default_factory=list)
    status: str = "pending"
    note: str = ""


@dataclasses.dataclass(frozen=True)
class TokenEncoder:
    name: str
    version: str
    encode: Callable[[str], list[int]]


def pick_token_encoder() -> TokenEncoder:
    try:
        import tiktoken
    except ImportError as exc:
        raise RuntimeError(
            "token counts require tiktoken; install it before comparing source tokens"
        ) from exc

    return TokenEncoder(
        name="cl100k_base",
        version=importlib.metadata.version("tiktoken"),
        encode=tiktoken.get_encoding("cl100k_base").encode,
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


def contract_from_path(contract_path: Path) -> dict[str, object]:
    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    if not isinstance(contract, dict):
        raise ValueError(f"{contract_path}: expected an object")
    if set(contract) == {"array_values"}:
        values = contract["array_values"]
        if (
            not isinstance(values, list)
            or len(values) != 3
            or any(type(value) is not int or not 0 <= value <= 255 for value in values)
        ):
            raise ValueError(f"{contract_path}: expected three byte-range array_values")
        return contract
    if set(contract) == {"kind", "next_indices"} and contract["kind"] == "pointer_chase":
        values = contract["next_indices"]
        if (
            not isinstance(values, list)
            or not 2 <= len(values) <= 256
            or any(type(value) is not int or not 0 <= value < len(values) for value in values)
        ):
            raise ValueError(f"{contract_path}: expected 2..256 in-range next_indices")
        visited: set[int] = set()
        index = 0
        for _ in values:
            if index in visited:
                raise ValueError(f"{contract_path}: pointer chase must visit every index")
            visited.add(index)
            index = values[index]
        if index != 0:
            raise ValueError(f"{contract_path}: pointer chase must return to zero")
        return contract
    if set(contract) == {"kind", "size", "stride"} and contract["kind"] == "generated_chase":
        size = contract["size"]
        stride = contract["stride"]
        if (
            type(size) is not int
            or type(stride) is not int
            or not 2 <= size <= 1_048_576
            or not 1 <= stride < size
            or math.gcd(size, stride) != 1
        ):
            raise ValueError(f"{contract_path}: expected coprime size and stride in range")
        return contract
    raise ValueError(f"{contract_path}: unknown benchmark contract")


def contract_for_folder(folder: Path) -> dict[str, object] | None:
    contract_path = folder / "contract.json"
    return contract_from_path(contract_path) if contract_path.exists() else None


def array_values_for_folder(folder: Path) -> list[int] | None:
    contract = contract_for_folder(folder)
    return contract["array_values"] if contract and "array_values" in contract else None


def run(
    command: list[str], *, cwd: Path | None = None, timeout: float | None = None
) -> tuple[int, str, str, int]:
    start = time.perf_counter_ns()
    result = subprocess.run(
        command, cwd=cwd, capture_output=True, text=True, check=False, timeout=timeout
    )
    elapsed = time.perf_counter_ns() - start
    return result.returncode, result.stdout, result.stderr, elapsed


def ensure_tool(name: str) -> str:
    path = shutil.which(name)
    if not path:
        raise FileNotFoundError(name)
    return path


def installed_tool_versions() -> dict[str, str | None]:
    versions: dict[str, str | None] = {}
    for name, arguments in {
        "cargo": ["--version"],
        "rustc": ["--version"],
        "node": ["--version"],
        "tsc": ["--version"],
        "go": ["version"],
        "dotnet": ["--version"],
        "c++": ["--version"],
    }.items():
        path = shutil.which(name)
        if path is None:
            versions[name] = None
            continue
        try:
            code, stdout, stderr, _ = run([path, *arguments], timeout=5)
            versions[name] = (
                (stdout or stderr).splitlines()[0] if code == 0 else None
            )
        except (OSError, subprocess.TimeoutExpired, IndexError):
            versions[name] = None
    return versions


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
    cargo = ensure_tool("cargo")
    code, _, stderr, _ = run(
        [cargo, "build", "-p", "tokit-compiler", "--bin", "tok"],
        cwd=ROOT,
    )
    if code:
        raise RuntimeError(f"Tokit compiler build failed: {stderr.strip()}")
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
                "    <EnableDefaultCompileItems>false</EnableDefaultCompileItems>",
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
    source: BenchSource, arguments: list[str], temp_dir: Path
) -> tuple[int | None, list[str] | None, int, str]:
    """Compile once and return compile ns, runtime command, exit code, and note."""

    source_path = source.path
    language = source.language
    compile_ns: int | None = None
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
        return_code, stdout, stderr, compile_ns = run(compile_command, cwd=ROOT)
        if return_code:
            return compile_ns, None, return_code, stderr.strip() or stdout.strip()
        runtime_command = [str(output), *arguments]
    elif language == "javascript":
        runtime_command = [ensure_tool("node"), str(source_path), *arguments]
    elif language == "typescript":
        tsc_command = [ensure_tool("tsc")]
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
        return_code, stdout, stderr, compile_ns = run(compile_command, cwd=ROOT)
        if return_code:
            return compile_ns, None, return_code, stderr.strip() or stdout.strip()
        runtime_command = [ensure_tool("node"), str(compiled), *arguments]
    elif language == "go":
        compiler = go_compiler()
        output = temp_dir / (source_path.stem + (".exe" if os.name == "nt" else ""))
        compile_command = [compiler, "build", "-o", str(output), str(source_path)]
        return_code, _, stderr, compile_ns = run(compile_command, cwd=ROOT)
        if return_code:
            return compile_ns, None, return_code, stderr.strip()
        runtime_command = [str(output), *arguments]
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
            return compile_ns, None, return_code, stderr.strip()
        runtime_command = [str(output), *arguments]
    elif language == "csharp":
        dotnet = ensure_tool("dotnet")
        version_code, version, _, _ = run([dotnet, "--version"])
        if version_code or int(version.strip().split(".", 1)[0]) < 8:
            raise FileNotFoundError("dotnet SDK 8 or newer")
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
        return_code, stdout, stderr, compile_ns = run(compile_command, cwd=ROOT)
        if return_code:
            return compile_ns, None, return_code, stderr.strip() or stdout.strip()
        runtime_command = [
            dotnet,
            str(project_dir / "out" / f"{source_path.stem}.dll"),
            *arguments,
        ]
    elif language == "python":
        runtime_command = [sys.executable, str(source_path), *arguments]
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
            return compile_ns, None, return_code, stderr.strip()
        runtime_command = [str(output), *arguments]
    else:
        raise ValueError(f"unsupported language: {language}")

    return compile_ns, runtime_command, return_code, note


def cycle_total(iterations: int, values: list[int] | None = None) -> int:
    if values is None:
        values = [1, 2, 3]
    cycles, remainder = divmod(iterations, 3)
    return sum(values) * cycles + sum(values[:remainder])


def expected_output(iterations: int, values: list[int] | None = None) -> str:
    return f"Ok({cycle_total(iterations, values)})"


def pointer_total(iterations: int, values: list[int]) -> int:
    route: list[int] = []
    index = 0
    for _ in values:
        index = values[index]
        route.append(index)
    cycles, remainder = divmod(iterations, len(route))
    return cycles * sum(route) + sum(route[:remainder])


def generated_total(iterations: int, size: int, stride: int) -> int:
    cycles, remainder = divmod(iterations, size)
    total = cycles * (size // 2)
    index = 0
    for _ in range(remainder):
        index = (index + stride) % size
        total += index < size // 2
    return total


def total_for_contract(iterations: int, contract: dict[str, object] | None) -> int:
    if contract and contract.get("kind") == "pointer_chase":
        return pointer_total(iterations, contract["next_indices"])
    if contract and contract.get("kind") == "generated_chase":
        return generated_total(iterations, contract["size"], contract["stride"])
    return cycle_total(iterations, contract["array_values"] if contract else None)


def expected_for_contract(iterations: int, contract: dict[str, object] | None) -> str:
    return f"Ok({total_for_contract(iterations, contract)})"


def arguments_for_contract(iterations: int, contract: dict[str, object] | None) -> list[str]:
    if contract is None:
        return [str(iterations)]
    if contract.get("kind") == "generated_chase":
        return [str(iterations), str(contract["size"]), str(contract["stride"])]
    values = contract.get("next_indices", contract.get("array_values", []))
    return [str(iterations), *map(str, values)]


def measure_runtime(
    command: list[str], expected: str, warmups: int, samples: int, timeout: float
) -> tuple[list[int], str]:
    times: list[int] = []
    for attempt in range(warmups + samples):
        code, stdout, stderr, elapsed = run(command, cwd=ROOT, timeout=timeout)
        if code != 0:
            raise RuntimeError(f"exit {code}: {stderr.strip() or stdout.strip()}")
        if stdout.strip() != expected:
            raise RuntimeError(f"wrong output: expected {expected!r}, got {stdout.strip()!r}")
        if attempt >= warmups:
            times.append(elapsed)
    return times, expected


def format_ns(value: int | None) -> str:
    if value is None:
        return "n/a"
    return f"{value / 1_000_000:.3f}"


def render_table(rows: list[Measurement]) -> str:
    headers = [
        "folder",
        "file",
        "lang",
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
            format_ns(row.compile_ns),
            format_ns(row.runtime_ns),
            str(row.source_tokens) if row.source_tokens is not None else "n/a",
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
    parser.add_argument("--iterations", type=int, default=300_000)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument(
        "--contract", type=Path, help="use this validated contract for one benchmark folder"
    )
    parser.add_argument(
        "--json", action="store_true", help="also print machine-readable JSON"
    )
    parser.add_argument("--output", type=Path, help="write raw JSON report to this file")
    args = parser.parse_args()

    patterns = args.patterns or DEFAULT_PATTERNS
    folders = detect_benchmark_folders(patterns)
    if not folders:
        raise SystemExit(f"no benchmark folders matched: {', '.join(patterns)}")
    if args.contract and len(folders) != 1:
        parser.error("--contract requires exactly one benchmark folder")
    if args.contract and not args.contract.is_file():
        parser.error(f"contract does not exist: {args.contract}")
    if not 0 <= args.iterations <= 1_073_741_823:
        raise SystemExit("iterations must be 0..1073741823 for the i32 result contract")
    if args.warmups < 0 or args.samples < 1 or args.timeout <= 0:
        raise SystemExit("warmups must be nonnegative; samples and timeout must be positive")

    try:
        token_encoder = pick_token_encoder()
    except RuntimeError as exc:
        token_encoder = None
        print(f"token counts unavailable: {exc}", file=sys.stderr)
    results: list[Measurement] = []
    contracts = {
        folder.name: contract_from_path(args.contract)
        if args.contract
        else contract_for_folder(folder)
        for folder in folders
    }

    with tempfile.TemporaryDirectory(prefix="tokit-benchmark-matrix-") as temp_root:
        temp_root_path = Path(temp_root)
        for folder in folders:
            contract = contracts[folder.name]
            total = total_for_contract(args.iterations, contract)
            expected = f"Ok({total})"
            if total > 2_147_483_647:
                raise SystemExit(f"{folder.name}: result exceeds i32")
            arguments = arguments_for_contract(args.iterations, contract)
            for source in collect_sources(folder):
                source_data = source.path.read_bytes()
                source_text = source_data.decode("utf-8")
                source_bytes = len(source_data)
                source_tokens = (
                    len(token_encoder.encode(source_text)) if token_encoder else None
                )

                measurement = Measurement(
                    folder=folder.name,
                    file=source.path.name,
                    language=source.language,
                    source_bytes=source_bytes,
                    source_sha256=hashlib.sha256(source_data).hexdigest(),
                    source_tokens=source_tokens,
                )

                try:
                    run_root = (
                        temp_root_path / folder.name / f"{source.path.stem}-{source.language}"
                    )
                    run_root.mkdir(parents=True, exist_ok=True)
                    compile_ns, runtime_command, code, note = compile_and_run(
                        source,
                        arguments,
                        run_root,
                    )
                    measurement.compile_ns = compile_ns
                    if code != 0:
                        measurement.status = f"build-exit:{code}"
                        measurement.note = note
                    else:
                        assert runtime_command is not None
                        samples, verified = measure_runtime(
                            runtime_command,
                            expected,
                            args.warmups,
                            args.samples,
                            args.timeout,
                        )
                        measurement.runtime_samples_ns = samples
                        measurement.runtime_ns = int(statistics.median(samples))
                        measurement.status = "ok"
                        measurement.note = verified
                except FileNotFoundError as exc:
                    measurement.status = "missing-tool"
                    measurement.note = str(exc)
                except (RuntimeError, subprocess.TimeoutExpired, OSError) as exc:
                    measurement.status = "error"
                    measurement.note = str(exc)

                results.append(measurement)

    tokenizer_label = (
        f"{token_encoder.name} (tiktoken {token_encoder.version})"
        if token_encoder else "unavailable"
    )
    print(f"tokenizer: {tokenizer_label}")
    print(
        f"iterations: {args.iterations}; warmups: {args.warmups}; "
        f"samples: {args.samples}; timeout_s: {args.timeout:g}"
    )
    print(render_table(results))
    for row in results:
        if row.status != "ok":
            print(f"{row.file}: {row.status}: {row.note}", file=sys.stderr)
    report = {
        "iterations": args.iterations,
        "warmups": args.warmups,
        "samples": args.samples,
        "timeout_s": args.timeout,
        "tokenizer": token_encoder.name if token_encoder else None,
        "tiktoken_version": token_encoder.version if token_encoder else None,
        "host": platform.platform(),
        "python": sys.version.split()[0],
        "tool_versions": installed_tool_versions(),
        "contracts": {
            name: contract if contract is not None else {"cycle": [1, 2, 3]}
            for name, contract in contracts.items()
        },
        "contract_override": str(args.contract) if args.contract else None,
        "complete": all(row.status == "ok" for row in results),
        "measurements": [dataclasses.asdict(row) for row in results],
    }
    if args.output:
        args.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    if args.json:
        print(json.dumps(report, indent=2, ensure_ascii=False))
    if any(row.status not in {"ok", "missing-tool"} for row in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
