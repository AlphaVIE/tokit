#include <cstdint>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

static int64_t sum_values(int32_t n, const std::vector<int64_t>& values) {
    std::size_t index = 0;
    int64_t total = 0;
    for (int32_t step = 0; step < n; ++step) {
        total += values.at(index);
        ++index;
        if (index == values.size()) index = 0;
    }
    return total;
}

int main(int argc, char** argv) {
    if (argc < 4) throw std::runtime_error("invalid arguments");
    const int32_t n = std::stoi(argv[1]);
    std::vector<int64_t> values;
    values.reserve(static_cast<std::size_t>(argc - 2));
    for (int at = 2; at < argc; ++at) values.push_back(std::stoll(argv[at]));
    std::cout << "Ok(" << sum_values(n, values) << ")\n";
}
