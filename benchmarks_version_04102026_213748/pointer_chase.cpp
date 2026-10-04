#include <cstdint>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

static int32_t walk(int32_t n, const std::vector<int32_t>& next) {
    int32_t index = 0;
    int32_t total = 0;
    for (int32_t step = 0; step < n; ++step) {
        index = next.at(static_cast<std::size_t>(index));
        total += index;
    }
    return total;
}

int main(int argc, char** argv) {
    if (argc < 4) {
        throw std::runtime_error("invalid arguments");
    }
    const int32_t n = std::stoi(argv[1]);
    std::vector<int32_t> next;
    next.reserve(static_cast<std::size_t>(argc - 2));
    for (int at = 2; at < argc; ++at) {
        next.push_back(std::stoi(argv[at]));
    }
    std::cout << "Ok(" << walk(n, next) << ")\n";
}
