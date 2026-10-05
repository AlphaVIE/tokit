"""Oracle checks for the record-field timing harness."""

import sys
import unittest

from scripts.benchmark_record_field import timed


class RecordFieldBenchmarkTests(unittest.TestCase):
    def test_rejects_wrong_output_and_failed_processes(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "wrong benchmark result"):
            timed([sys.executable, "-c", "print('Ok(14)')"], "Ok(21)")
        with self.assertRaisesRegex(RuntimeError, "wrong benchmark result"):
            timed([sys.executable, "-c", "print('Ok(21)'); raise SystemExit(1)"], "Ok(21)")

    def test_accepts_exact_output(self) -> None:
        self.assertGreaterEqual(timed([sys.executable, "-c", "print('Ok(21)')"], "Ok(21)"), 0)


if __name__ == "__main__":
    unittest.main()
