import sys


def walk(n: int, size: int, stride: int) -> int:
    next_indices = []
    for index in range(size):
        value = index + stride
        next_indices.append(value - size if value >= size else value)
    index = 0
    total = 0
    half = size // 2
    for _ in range(n):
        index = next_indices[index]
        if index < half:
            total += 1
    return total


if len(sys.argv) != 4:
    raise SystemExit("invalid arguments")
print(f"Ok({walk(*map(int, sys.argv[1:]))})")
