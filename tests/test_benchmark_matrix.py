"""Contract checks for the versioned benchmark harness."""

import sys
import tempfile
import unittest
from pathlib import Path

from scripts.benchmark_matrix import (
    arguments_for_contract,
    array_values_for_folder,
    contract_for_folder,
    expected_for_contract,
    expected_output,
    generated_total,
    measure_runtime,
    wide_sum_total,
)


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

    def test_pointer_contract_requires_a_full_runtime_cycle(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            contract_path = folder / "contract.json"
            contract_path.write_text(
                '{"kind":"pointer_chase","next_indices":[1,2,3,0]}', encoding="utf-8"
            )
            contract = contract_for_folder(folder)
            self.assertEqual(expected_for_contract(7, contract), "Ok(12)")
            self.assertEqual(arguments_for_contract(7, contract), ["7", "1", "2", "3", "0"])
            contract_path.write_text(
                '{"kind":"pointer_chase","next_indices":[1,1,0]}', encoding="utf-8"
            )
            with self.assertRaisesRegex(ValueError, "visit every index"):
                contract_for_folder(folder)
            contract_path.write_text(
                '{"kind":"pointer_chase","next_indices":[1,3,0]}', encoding="utf-8"
            )
            with self.assertRaisesRegex(ValueError, "in-range"):
                contract_for_folder(folder)

    def test_generated_chase_contract_and_oracle(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            contract_path = folder / "contract.json"
            contract_path.write_text(
                '{"kind":"generated_chase","size":4,"stride":1}', encoding="utf-8"
            )
            contract = contract_for_folder(folder)
            self.assertEqual(expected_for_contract(7, contract), "Ok(3)")
            self.assertEqual(arguments_for_contract(7, contract), ["7", "4", "1"])
            contract_path.write_text(
                '{"kind":"generated_chase","size":4,"stride":2}', encoding="utf-8"
            )
            with self.assertRaisesRegex(ValueError, "coprime"):
                contract_for_folder(folder)

        for size, stride in [(4, 1), (4, 3), (5, 2)]:
            for iterations in range(size * 2 + 3):
                index = 0
                direct = 0
                for _ in range(iterations):
                    index = (index + stride) % size
                    direct += index < size // 2
                self.assertEqual(generated_total(iterations, size, stride), direct)

    def test_wide_sum_contract_and_oracle(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            contract_path = folder / "contract.json"
            contract_path.write_text(
                '{"kind":"wide_sum","values":[3000000000,4000000000,5000000000]}',
                encoding="utf-8",
            )
            contract = contract_for_folder(folder)
            self.assertEqual(expected_for_contract(7, contract), "Ok(27000000000)")
            self.assertEqual(arguments_for_contract(7, contract), ["7", "3000000000", "4000000000", "5000000000"])
            self.assertEqual(wide_sum_total(0, contract["values"]), 0)
            contract_path.write_text(
                '{"kind":"wide_sum","values":[3000000000,true]}', encoding="utf-8"
            )
            with self.assertRaisesRegex(ValueError, "positive i64"):
                contract_for_folder(folder)

    def test_runtime_measurement_keeps_all_verified_samples(self) -> None:
        command = [sys.executable, "-c", "print('Ok(6)')"]
        samples, output = measure_runtime(command, "Ok(6)", warmups=1, samples=2, timeout=5)
        self.assertEqual(output, "Ok(6)")
        self.assertEqual(len(samples), 2)
        self.assertTrue(all(sample > 0 for sample in samples))


if __name__ == "__main__":
    unittest.main()
