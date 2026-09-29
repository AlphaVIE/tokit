#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

// BENCH_START
typedef enum { DivZero } DivError;
typedef struct { int ok; int32_t value; DivError error; } Result;
Result divide(int32_t a, int32_t b) {
    if (b == 0) return (Result){.ok = 0, .error = DivZero};
    return (Result){.ok = 1, .value = a / b};
}
// BENCH_END

int main(int argc, char **argv) {
    if (argc != 3) return 2;
    int32_t a = (int32_t)strtol(argv[1], NULL, 10);
    int32_t b = (int32_t)strtol(argv[2], NULL, 10);
    Result result = divide(a, b);
    if (result.ok) printf("Ok(%d)\n", result.value);
    else puts("Err(DivError::DivZero)");
    return 0;
}
