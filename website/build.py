"""Build the static site in website/public from website/src.

Tokit snippets are highlighted at build time with the real lexer
(`tok tokens`), so the published page ships no highlighter. Every snippet is
also type-checked, so the website cannot show code that does not compile.

  python website/build.py --tok target/release/tok
"""

from __future__ import annotations

import argparse
import html
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE = HERE / "src"
OUTPUT = HERE / "public"

KEYWORDS = {"Fn", "Struct", "Enum", "Import", "Pub", "Let", "Var", "For", "While", "Break", "Continue",
            "In", "If", "Match", "Spawn", "Else", "Return"}
CONSTANTS = {"True", "False", "Ok", "Err", "Some", "None"}
TYPES = {"I", "L", "F", "i32", "i64", "f64", "bool", "String", "Bytes", "Unit", "Result", "Option", "Map",
         "Task", "Request", "Response", "Process", "Conn", "Listener", "IoError", "ParseError"}


def highlight(tok: Path, code: str, check: bool) -> str:
    with tempfile.TemporaryDirectory() as work:
        path = Path(work) / "snippet.tok"
        path.write_text(code, encoding="utf-8", newline="\n")
        if check:
            checked = subprocess.run([str(tok), "check", str(path)], capture_output=True, text=True)
            if checked.returncode != 0:
                raise SystemExit(f"snippet does not check: {checked.stdout}{checked.stderr}\n{code}")
        listing = subprocess.run([str(tok), "tokens", str(path)], capture_output=True, text=True,
                                 check=True).stdout.split("\n")
    data = code.encode("utf-8")
    tokens = []
    for line in listing:
        parts = line.split()
        if len(parts) == 3 and parts[0] != "Eof":
            tokens.append((parts[0], int(parts[1]), int(parts[2])))
    out, position = [], 0

    def gap(text: str) -> str:
        # Comments are the only non-whitespace text the lexer skips.
        return re.sub(r"(//[^\n]*)", r'<span class="c">\1</span>', html.escape(text))

    for index, (kind, start, end) in enumerate(tokens):
        out.append(gap(data[position:start].decode("utf-8")))
        text = data[start:end].decode("utf-8")
        following = tokens[index + 1][0] if index + 1 < len(tokens) else ""
        if kind in KEYWORDS:
            css = "k"
        elif kind in CONSTANTS:
            css = "v"
        elif kind == "String":
            css = "s"
        elif kind in {"Int", "Int64", "Float64"}:
            css = "n"
        elif kind == "Ident" and (text in TYPES or text[:1].isupper()):
            css = "t"
        elif kind == "Ident" and following == "LParen":
            css = "f"
        else:
            css = ""
        escaped = html.escape(text)
        out.append(f'<span class="{css}">{escaped}</span>' if css else escaped)
        position = end
    out.append(gap(data[position:].decode("utf-8")))
    return "".join(out)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--tok", type=Path, required=True)
    args = parser.parse_args()
    page = (SOURCE / "index.html").read_text(encoding="utf-8")

    def replace(match: re.Match) -> str:
        name, check = match.group(1), match.group(2) != "nocheck"
        code = (SOURCE / "snippets" / f"{name}.tok").read_text(encoding="utf-8").rstrip("\n")
        return highlight(args.tok, code, check)

    page = re.sub(r"<!--tokit:([\w-]+)(?::(\w+))?-->", replace, page)
    if OUTPUT.exists():
        shutil.rmtree(OUTPUT)
    OUTPUT.mkdir()
    (OUTPUT / "index.html").write_text(page, encoding="utf-8", newline="\n")
    for asset in ("style.css", "favicon.svg", "404.html", "robots.txt"):
        shutil.copy(SOURCE / asset, OUTPUT / asset)
    total = sum(path.stat().st_size for path in OUTPUT.iterdir())
    print(f"built {OUTPUT} ({total / 1024:.1f} KiB)")


if __name__ == "__main__":
    main()
