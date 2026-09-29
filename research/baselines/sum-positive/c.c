#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

// BENCH_START
int32_t sum_positive(const int32_t *xs, size_t len) {
    int32_t total = 0;
    for (size_t i = 0; i < len; ++i) {
        if (xs[i] > 0) total += xs[i];
    }
    return total;
}
// BENCH_END

int main(int argc, char **argv) {
    size_t len = (size_t)(argc - 1);
    int32_t *xs = malloc((len ? len : 1) * sizeof *xs);
    if (!xs) return 2;
    for (size_t i = 0; i < len; ++i) {
        char *end;
        long n = strtol(argv[i + 1], &end, 10);
        if (*end || n < INT32_MIN || n > INT32_MAX) { free(xs); return 2; }
        xs[i] = (int32_t)n;
    }
    printf("%d\n", sum_positive(xs, len));
    free(xs);
    return 0;
}
