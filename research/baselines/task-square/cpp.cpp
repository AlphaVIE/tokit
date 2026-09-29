#include <cstdint>
#include <cstdlib>
#include <future>
#include <iostream>

// BENCH_START
static int32_t square_async(int32_t x) {
    return std::async(std::launch::async, [x] { return x * x; }).get();
}
// BENCH_END

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    try { std::cout << "Ok(" << square_async(static_cast<int32_t>(std::strtol(argv[1], nullptr, 10))) << ")\n"; }
    catch (...) { std::cout << "Err(TaskError::Failed)\n"; }
}
