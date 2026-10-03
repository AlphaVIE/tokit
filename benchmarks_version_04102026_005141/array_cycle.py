import sys


def array_sum(n: int, values: list[int]) -> int:
    index = 0
    total = 0
    for _ in range(n):
        total += values[index]
        index = 0 if index == len(values) - 1 else index + 1
    return total


if len(sys.argv) != 5:
    raise SystemExit("invalid arguments")

print(f"Ok({array_sum(int(sys.argv[1]), [int(value) for value in sys.argv[2:]])})")
