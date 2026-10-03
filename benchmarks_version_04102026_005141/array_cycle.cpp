#include <cstdint>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

static int32_t array_sum(int32_t n, const std::vector<int32_t>& values) {
    std::size_t index = 0;
    int32_t total = 0;
    for (int32_t i = 0; i < n; ++i) {
        total += values.at(index);
        index = index == values.size() - 1 ? 0 : index + 1;
    }
    return total;
}

int main(int argc, char** argv) {
    if (argc != 5) {
        throw std::runtime_error("invalid arguments");
    }
    const int32_t n = std::stoi(argv[1]);
    const std::vector<int32_t> values{
        std::stoi(argv[2]), std::stoi(argv[3]), std::stoi(argv[4])
    };
    std::cout << "Ok(" << array_sum(n, values) << ")\n";
}
