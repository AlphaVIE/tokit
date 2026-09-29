// BENCH_START
type DivError = "DivZero";
type Result = { ok: true; value: number } | { ok: false; error: DivError };
function divide(a: number, b: number): Result {
    if (b === 0) return { ok: false, error: "DivZero" };
    return { ok: true, value: Math.trunc(a / b) };
}
// BENCH_END

const result = divide(Number(process.argv[2]), Number(process.argv[3]));
console.log(result.ok ? `Ok(${result.value})` : "Err(DivError::DivZero)");
