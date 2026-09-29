#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <variant>

// BENCH_START
enum class DivError { DivZero };
std::variant<int32_t, DivError> divide(int32_t a, int32_t b) {
    if (b == 0) return DivError::DivZero;
    return a / b;
}
// BENCH_END

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    auto result = divide(static_cast<int32_t>(std::strtol(argv[1], nullptr, 10)),
                         static_cast<int32_t>(std::strtol(argv[2], nullptr, 10)));
    if (std::holds_alternative<int32_t>(result)) std::cout << "Ok(" << std::get<int32_t>(result) << ")\n";
    else std::cout << "Err(DivError::DivZero)\n";
}
