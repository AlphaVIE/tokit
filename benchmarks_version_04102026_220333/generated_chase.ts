declare const process: { argv: string[] };

function walk(n: number, size: number, stride: number): number {
  const next: number[] = [];
  for (let i = 0; i < size; i += 1) {
    const value = i + stride;
    next.push(value >= size ? value - size : value);
  }
  let index = 0;
  let total = 0;
  const half = Math.floor(size / 2);
  for (let step = 0; step < n; step += 1) {
    index = next[index];
    if (index < half) total += 1;
  }
  return total;
}

const args = process.argv.slice(2).map(Number);
if (args.length !== 3 || !args.every(Number.isInteger)) throw new Error("invalid arguments");
console.log(`Ok(${walk(...args)})`);
