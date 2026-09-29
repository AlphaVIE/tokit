from enum import Enum
import sys


# BENCH_START
class DivError(Enum):
    DivZero = 1


def divide(a: int, b: int) -> tuple[bool, int | DivError]:
    if b == 0:
        return False, DivError.DivZero
    quotient = abs(a) // abs(b)
    return True, quotient if (a < 0) == (b < 0) else -quotient
# BENCH_END


ok, value = divide(int(sys.argv[1]), int(sys.argv[2]))
print(f"Ok({value})" if ok else "Err(DivError::DivZero)")
