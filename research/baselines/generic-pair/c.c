#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// BENCH_START
#define DEFINE_PAIR(T, NAME) \
    typedef struct { T left; T right; } Pair##NAME; \
    Pair##NAME flip##NAME(Pair##NAME p) { return (Pair##NAME){p.right, p.left}; }
DEFINE_PAIR(int32_t, Int)
DEFINE_PAIR(bool, Bool)
DEFINE_PAIR(const char *, String)
// BENCH_END

int main(int argc, char **argv) {
    if (argc != 4) return 2;
    if (strcmp(argv[1], "i32") == 0) {
        PairInt p = flipInt((PairInt){(int32_t)strtol(argv[2], NULL, 10), (int32_t)strtol(argv[3], NULL, 10)});
        printf("Pair(left:%d,right:%d)\n", p.left, p.right);
    } else if (strcmp(argv[1], "bool") == 0) {
        PairBool p = flipBool((PairBool){strcmp(argv[2], "true") == 0, strcmp(argv[3], "true") == 0});
        printf("Pair(left:%s,right:%s)\n", p.left ? "true" : "false", p.right ? "true" : "false");
    } else if (strcmp(argv[1], "String") == 0) {
        PairString p = flipString((PairString){argv[2], argv[3]});
        printf("Pair(left:\"%s\",right:\"%s\")\n", p.left, p.right);
    } else return 2;
    return 0;
}
