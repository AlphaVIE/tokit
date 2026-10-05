declare const process: { argv: string[] };

function sumValues(n: number, values: bigint[]): bigint {
  let index = 0;
  let total = 0n;
  for (let step = 0; step < n; step += 1) {
    total += values[index];
    index += 1;
    if (index === values.length) index = 0;
  }
  return total;
}

const args = process.argv.slice(2);
if (args.length < 3) throw new Error("invalid arguments");
const n = Number(args[0]);
if (!Number.isInteger(n)) throw new Error("invalid iteration count");
const values = args.slice(1).map(BigInt);
console.log(`Ok(${sumValues(n, values)})`);
