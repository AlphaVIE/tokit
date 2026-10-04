function walk(n, next) {
  let index = 0;
  let total = 0;
  for (let step = 0; step < n; step += 1) {
    index = next[index];
    total += index;
  }
  return total;
}

const args = process.argv.slice(2);
if (args.length < 3) {
  throw new Error("invalid arguments");
}
const values = args.map(Number);
if (!values.every(Number.isInteger)) {
  throw new Error("invalid integer");
}
console.log(`Ok(${walk(values[0], values.slice(1))})`);
