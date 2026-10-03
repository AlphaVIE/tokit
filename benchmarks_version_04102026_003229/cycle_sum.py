import sys


def cycle_sum(n: int) -> int:
    value = 1
    total = 0
    for _ in range(n):
        total += value
        value = 1 if value == 3 else value + 1
    return total


if len(sys.argv) != 2:
    raise SystemExit("invalid arguments")

print(f"Ok({cycle_sum(int(sys.argv[1]))})")
