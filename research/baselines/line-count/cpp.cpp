#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <optional>
#include <stdexcept>
#include <string>

// BENCH_START
std::optional<std::int32_t> line_count(const std::string& path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        if (!std::filesystem::exists(path)) return std::nullopt;
        throw std::runtime_error("cannot read file");
    }
    std::int32_t count = 0;
    char byte = 0;
    char last = 0;
    bool any = false;
    while (file.get(byte)) {
        if (byte == '\n') ++count;
        last = byte;
        any = true;
    }
    if (file.bad()) throw std::runtime_error("read failed");
    if (any && last != '\n') ++count;
    return count;
}
// BENCH_END

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    auto count = line_count(argv[1]);
    if (count) std::cout << "Ok(" << *count << ")\n";
    else std::cout << "Err(IoError::NotFound)\n";
}
