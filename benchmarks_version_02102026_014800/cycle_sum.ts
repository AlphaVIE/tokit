function cycleSum(n: number): number {
  let value = 1;
  let total = 0;
  for (let i = 0; i < n; i += 1) {
    total += value;
    value = value === 3 ? 1 : value + 1;
  }
  return total;
}

const args = process.argv.slice(2);

if (args.length !== 1) {
  throw new Error("invalid arguments");
}

const n = Number.parseInt(args[0], 10);

if (!Number.isInteger(n)) {
  throw new Error("invalid integer");
}

console.log(`Ok(${cycleSum(n)})`);
