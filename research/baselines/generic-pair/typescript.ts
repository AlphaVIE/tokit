// BENCH_START
type Pair<T> = { left: T; right: T };
function flip<T>(p: Pair<T>): Pair<T> { return { left: p.right, right: p.left }; }
// BENCH_END

const [kind, a, b] = process.argv.slice(2);
if (kind === "i32") {
    const p = flip({ left: Number(a), right: Number(b) });
    console.log(`Pair(left:${p.left},right:${p.right})`);
} else if (kind === "bool") {
    const p = flip({ left: a === "true", right: b === "true" });
    console.log(`Pair(left:${p.left},right:${p.right})`);
} else if (kind === "String") {
    const p = flip({ left: a, right: b });
    console.log(`Pair(left:${JSON.stringify(p.left)},right:${JSON.stringify(p.right)})`);
} else throw new Error("unknown type");
