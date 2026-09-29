#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

// BENCH_START
int32_t line_count(const char *path, int *missing) {
    FILE *file = fopen(path, "rb");
    if (!file) {
        if (errno == ENOENT) { *missing = 1; return 0; }
        perror("fopen"); exit(2);
    }
    int32_t count = 0;
    int last = EOF;
    int byte;
    while ((byte = fgetc(file)) != EOF) {
        if (byte == '\n') count++;
        last = byte;
    }
    if (ferror(file)) { perror("fgetc"); fclose(file); exit(2); }
    fclose(file);
    if (last != EOF && last != '\n') count++;
    return count;
}
// BENCH_END

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    int missing = 0;
    int32_t count = line_count(argv[1], &missing);
    if (missing) puts("Err(IoError::NotFound)");
    else printf("Ok(%d)\n", count);
    return 0;
}
