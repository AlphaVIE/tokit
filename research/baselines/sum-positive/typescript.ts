// BENCH_START
function sumPositive(xs: number[]): number {
    let total = 0;
    for (const x of xs) {
        if (x > 0) total += x;
    }
    return total;
}
// BENCH_END

console.log(sumPositive(process.argv.slice(2).map(Number)));
