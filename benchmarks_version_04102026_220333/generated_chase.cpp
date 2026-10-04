#include <cstdint>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

static int32_t walk(int32_t n, int32_t size, int32_t stride) {
    std::vector<int32_t> next;
    next.reserve(static_cast<std::size_t>(size));
    for (int32_t i = 0; i < size; ++i) {
        const int32_t value = i + stride;
        next.push_back(value >= size ? value - size : value);
    }
    int32_t index = 0;
    int32_t total = 0;
    const int32_t half = size / 2;
    for (int32_t step = 0; step < n; ++step) {
        index = next.at(static_cast<std::size_t>(index));
        if (index < half) ++total;
    }
    return total;
}

int main(int argc, char** argv) {
    if (argc != 4) throw std::runtime_error("invalid arguments");
    std::cout << "Ok(" << walk(std::stoi(argv[1]), std::stoi(argv[2]), std::stoi(argv[3])) << ")\n";
}
