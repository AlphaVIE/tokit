"""Contract checks for the versioned benchmark harness."""

import sys
import tempfile
import unittest
from pathlib import Path

from scripts.benchmark_matrix import array_values_for_folder, expected_output, measure_runtime


class BenchmarkMatrixTests(unittest.TestCase):
    def test_cycle_sum_oracle_handles_complete_cycles_and_remainders(self) -> None:
        self.assertEqual(
            [expected_output(count) for count in range(7)],
            ["Ok(0)", "Ok(1)", "Ok(3)", "Ok(6)", "Ok(7)", "Ok(9)", "Ok(12)"],
        )
        self.assertEqual(expected_output(7, [2, 5, 9]), "Ok(34)")

    def test_runtime_measurement_rejects_wrong_output(self) -> None:
        command = [sys.executable, "-c", "print('Ok(7)')"]
        with self.assertRaisesRegex(RuntimeError, "wrong output"):
            measure_runtime(command, "Ok(6)", warmups=0, samples=1, timeout=5)

    def test_array_contract_requires_three_checked_values(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            contract = folder / "contract.json"
            self.assertIsNone(array_values_for_folder(folder))
            contract.write_text('{"array_values":[2,5,9]}', encoding="utf-8")
            self.assertEqual(array_values_for_folder(folder), [2, 5, 9])
            contract.write_text('{"array_values":[1,256,3]}', encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "byte-range"):
                array_values_for_folder(folder)

    def test_runtime_measurement_keeps_all_verified_samples(self) -> None:
        command = [sys.executable, "-c", "print('Ok(6)')"]
        samples, output = measure_runtime(command, "Ok(6)", warmups=1, samples=2, timeout=5)
        self.assertEqual(output, "Ok(6)")
        self.assertEqual(len(samples), 2)
        self.assertTrue(all(sample > 0 for sample in samples))


if __name__ == "__main__":
    unittest.main()
