"""Guard byte/character accounting in the research measurement tool."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from token_cost import attach_stats, measure  # noqa: E402


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

    def test_structural_ratios_keep_zero_denominators_explicit(self) -> None:
        measurement = {"bytes": 5, "chars": 5, "tokens": {"fixture": 8}}
        stats = {
            "bytes": 5, "chars": 5, "ast_nodes": 4,
            "semantic_ops": 2, "functions": 1,
            "declarations": 1, "dependencies": 0,
        }
        attach_stats(measurement, stats)
        self.assertEqual(measurement["structure"], stats)
        self.assertEqual(measurement["token_ratios"]["fixture"], {
            "per_ast_node": 2.0,
            "per_semantic_op": 4.0,
            "per_function": 8.0,
            "per_declaration": 8.0,
            "per_dependency": None,
        })


if __name__ == "__main__":
    unittest.main()
