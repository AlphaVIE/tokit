"""Guard byte/character accounting in the research measurement tool."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from token_cost import measure  # noqa: E402


class TokenCostTests(unittest.TestCase):
    def test_utf8_bytes_and_codepoints_are_distinct(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sample.tok.txt"
            path.write_bytes("aé\n".encode("utf-8"))
            result = measure(path, {"fixture": lambda source: source.splitlines()})

        self.assertEqual(result["bytes"], 4)
        self.assertEqual(result["chars"], 3)
        self.assertEqual(result["tokens"], {"fixture": 1})

    def test_core_region_excludes_execution_wrapper(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sample.py"
            path.write_bytes(b"before\n# BENCH_START\ncore\n# BENCH_END\nafter\n")
            result = measure(path, {"fixture": lambda source: source.splitlines()}, "core")
        self.assertEqual(result["bytes"], 5)
        self.assertEqual(result["chars"], 5)
        self.assertEqual(result["tokens"], {"fixture": 1})

    def test_core_region_requires_unique_markers(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sample.py"
            path.write_text("# BENCH_START\nx\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                measure(path, {}, "core")


if __name__ == "__main__":
    unittest.main()
