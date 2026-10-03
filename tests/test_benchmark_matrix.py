"""Contract checks for the versioned benchmark harness."""

import sys
import unittest

from scripts.benchmark_matrix import expected_output, measure_runtime


class BenchmarkMatrixTests(unittest.TestCase):
    def test_cycle_sum_oracle_handles_complete_cycles_and_remainders(self) -> None:
        self.assertEqual(
            [expected_output(count) for count in range(7)],
            ["Ok(0)", "Ok(1)", "Ok(3)", "Ok(6)", "Ok(7)", "Ok(9)", "Ok(12)"],
        )

    def test_runtime_measurement_rejects_wrong_output(self) -> None:
        command = [sys.executable, "-c", "print('Ok(7)')"]
        with self.assertRaisesRegex(RuntimeError, "wrong output"):
            measure_runtime(command, 3, warmups=0, samples=1, timeout=5)

    def test_runtime_measurement_keeps_all_verified_samples(self) -> None:
        command = [sys.executable, "-c", "print('Ok(6)')"]
        samples, output = measure_runtime(command, 3, warmups=1, samples=2, timeout=5)
        self.assertEqual(output, "Ok(6)")
        self.assertEqual(len(samples), 2)
        self.assertTrue(all(sample > 0 for sample in samples))


if __name__ == "__main__":
    unittest.main()
