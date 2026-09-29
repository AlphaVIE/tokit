#include <cstdint>
#include <iostream>
#include <string>
#include <vector>

// BENCH_START
int32_t sum_positive(const std::vector<int32_t>& xs) {
    int32_t total = 0;
    for (int32_t x : xs) {
        if (x > 0) total += x;
    }
    return total;
}
// BENCH_END

int main(int argc, char** argv) {
    std::vector<int32_t> xs;
    for (int i = 1; i < argc; ++i) xs.push_back(std::stoi(argv[i]));
    std::cout << sum_positive(xs) << '\n';
}
