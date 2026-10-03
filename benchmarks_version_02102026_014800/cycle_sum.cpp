#include <cstdint>
#include <iostream>
#include <stdexcept>
#include <string>

static int32_t cycle_sum(int32_t n) {
    int32_t value = 1;
    int32_t total = 0;
    for (int32_t i = 0; i < n; ++i) {
        total += value;
        value = value == 3 ? 1 : value + 1;
    }
    return total;
}

int main(int argc, char** argv) {
    if (argc != 2) {
        throw std::runtime_error("invalid arguments");
    }
    const int32_t n = std::stoi(argv[1]);
    std::cout << "Ok(" << cycle_sum(n) << ")\n";
}