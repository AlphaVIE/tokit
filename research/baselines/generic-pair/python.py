from dataclasses import dataclass
from typing import Generic, TypeVar
import sys


# BENCH_START
T = TypeVar("T")
@dataclass(frozen=True)
class Pair(Generic[T]):
    left: T
    right: T

def flip(p: Pair[T]) -> Pair[T]:
    return Pair(p.right, p.left)
# BENCH_END


kind, left, right = sys.argv[1:4]
if kind == "i32":
    left, right = int(left), int(right)
elif kind == "bool":
    left, right = left == "true", right == "true"
p = flip(Pair(left, right))
render = lambda value: '"' + value + '"' if isinstance(value, str) else str(value).lower()
print(f"Pair(left:{render(p.left)},right:{render(p.right)})")
