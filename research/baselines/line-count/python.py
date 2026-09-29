from pathlib import Path
from sys import argv

# BENCH_START
def line_count(path: str) -> int:
    text = Path(path).read_bytes().decode("utf-8")
    return text.count("\n") + int(bool(text) and not text.endswith("\n"))
# BENCH_END

try:
    print(f"Ok({line_count(argv[1])})")
except FileNotFoundError:
    print("Err(IoError::NotFound)")
