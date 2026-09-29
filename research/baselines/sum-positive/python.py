from sys import argv

# BENCH_START
def sum_positive(xs: list[int]) -> int:
    total = 0
    for x in xs:
        if x > 0:
            total += x
    return total
# BENCH_END

print(sum_positive([int(x) for x in argv[1:]]))
