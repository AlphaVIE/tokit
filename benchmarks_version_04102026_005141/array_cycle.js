function arraySum(n, values) {
  let index = 0;
  let total = 0;
  for (let i = 0; i < n; i += 1) {
    total += values[index];
    index = index === values.length - 1 ? 0 : index + 1;
  }
  return total;
}

const args = process.argv.slice(2);

if (args.length !== 4) {
  throw new Error("invalid arguments");
}

const n = Number.parseInt(args[0], 10);

if (!Number.isInteger(n)) {
  throw new Error("invalid integer");
}

const values = args.slice(1).map(Number);
if (!values.every(Number.isInteger)) {
  throw new Error("invalid array value");
}
console.log(`Ok(${arraySum(n, values)})`);
