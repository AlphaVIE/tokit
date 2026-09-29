from concurrent.futures import ThreadPoolExecutor
import sys


# BENCH_START
def square_async(x: int) -> int:
    with ThreadPoolExecutor(max_workers=1) as pool:
        return pool.submit(lambda: x * x).result()
# BENCH_END


try:
    print(f"Ok({square_async(int(sys.argv[1]))})")
except Exception:
    print("Err(TaskError::Failed)")
