import sys


def walk(n: int, next_indices: list[int]) -> int:
    index = 0
    total = 0
    for _ in range(n):
        index = next_indices[index]
        total += index
    return total


if len(sys.argv) < 4:
    raise SystemExit("invalid arguments")

print(f"Ok({walk(int(sys.argv[1]), [int(value) for value in sys.argv[2:]])})")
