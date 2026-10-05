import sys


def sum_values(n: int, values: list[int]) -> int:
    index = 0
    total = 0
    for _ in range(n):
        total += values[index]
        index += 1
        if index == len(values):
            index = 0
    return total


if len(sys.argv) < 4:
    raise SystemExit("invalid arguments")
print(f"Ok({sum_values(int(sys.argv[1]), [int(value) for value in sys.argv[2:]])})")
