#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <threads.h>

typedef struct { int32_t input; int32_t output; } Job;

// BENCH_START
static int square(void *raw) {
    Job *job = raw;
    job->output = job->input * job->input;
    return 0;
}
static int square_async(int32_t input, int32_t *output) {
    Job job = {input, 0};
    thrd_t thread;
    if (thrd_create(&thread, square, &job) != thrd_success) return -1;
    int status;
    if (thrd_join(thread, &status) != thrd_success || status != 0) return -1;
    *output = job.output;
    return 0;
}
// BENCH_END

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    int32_t output;
    if (square_async((int32_t)strtol(argv[1], NULL, 10), &output) != 0)
        puts("Err(TaskError::Failed)");
    else printf("Ok(%d)\n", output);
    return 0;
}
