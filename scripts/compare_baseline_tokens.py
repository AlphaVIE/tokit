"""Compare source tokens of the executable baselines across languages.

Counts only the core region between BENCH_START and BENCH_END of every
implementation in research/baselines (argument parsing and output wrappers
differ by language and are excluded). Tokit cores are first put into
canonical compact form with `tok compact`, which is how Tokit is written.

  python scripts/compare_baseline_tokens.py --tok target/release/tok
"""

from __future__ import annotations

import argparse
import re
import subprocess
import tempfile
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LANGUAGES = {".tok": "Tokit", ".rs": "Rust", ".py": "Python", ".go": "Go", ".ts": "TypeScript",
             ".java": "Java", ".c": "C", ".cpp": "C++"}
CORE = re.compile(r"BENCH_START[^\n]*\n(.*?)\n[^\n]*BENCH_END", re.S)


def cores(tok: Path) -> dict[str, dict[str, str]]:
    found: dict[str, dict[str, str]] = {}
    with tempfile.TemporaryDirectory() as work:
        for directory in sorted((ROOT / "research" / "baselines").iterdir()):
            if not directory.is_dir():
                continue
            for path in sorted(directory.iterdir()):
                language = LANGUAGES.get(path.suffix)
                match = CORE.search(path.read_text(encoding="utf-8")) if language else None
                if not match:
                    continue
                core = match.group(1)
                if language == "Tokit":
                    scratch = Path(work) / "core.tok"
                    scratch.write_text(core, encoding="utf-8", newline="\n")
                    compact = subprocess.run([str(tok), "compact", str(scratch)], capture_output=True,
                                             text=True, encoding="utf-8").stdout.strip()
                    core = compact or core
                found.setdefault(directory.name, {})[language] = core
    return found


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--tok", type=Path, required=True)
    args = parser.parse_args()
    import tiktoken

    sources = cores(args.tok)
    for name in ("o200k_base", "cl100k_base"):
        encoding = tiktoken.get_encoding(name)
        totals: Counter[str] = Counter()
        for tasks in sources.values():
            for language, core in tasks.items():
                totals[language] += len(encoding.encode(core))
        print(f"{name} ({len(sources)} tasks)")
        for language, count in sorted(totals.items(), key=lambda item: item[1]):
            saving = "" if language == "Tokit" else f"  Tokit uses {1 - totals['Tokit'] / count:.0%} fewer"
            print(f"  {language:11s} {count:5d}{saving}")


if __name__ == "__main__":
    main()
