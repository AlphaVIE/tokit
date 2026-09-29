#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <string>

// BENCH_START
template<class T> struct Pair { T left, right; };
template<class T> Pair<T> flip(Pair<T> p) { return {p.right, p.left}; }
// BENCH_END

int main(int argc, char **argv) {
    if (argc != 4) return 2;
    std::string kind = argv[1];
    if (kind == "i32") {
        auto p = flip(Pair<int32_t>{static_cast<int32_t>(std::strtol(argv[2], nullptr, 10)), static_cast<int32_t>(std::strtol(argv[3], nullptr, 10))});
        std::cout << "Pair(left:" << p.left << ",right:" << p.right << ")\n";
    } else if (kind == "bool") {
        auto p = flip(Pair<bool>{std::string(argv[2]) == "true", std::string(argv[3]) == "true"});
        std::cout << "Pair(left:" << (p.left ? "true" : "false") << ",right:" << (p.right ? "true" : "false") << ")\n";
    } else if (kind == "String") {
        auto p = flip(Pair<std::string>{argv[2], argv[3]});
        std::cout << "Pair(left:\"" << p.left << "\",right:\"" << p.right << "\")\n";
    } else return 2;
}
