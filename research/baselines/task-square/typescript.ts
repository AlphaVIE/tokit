import { Worker } from "node:worker_threads";

// BENCH_START
function squareAsync(x: number): Promise<number> {
    return new Promise((resolve, reject) => {
        const worker = new Worker(`const { parentPort, workerData } = require('node:worker_threads'); parentPort.postMessage(workerData * workerData);`, { eval: true, workerData: x });
        worker.once("message", resolve);
        worker.once("error", reject);
        worker.once("exit", code => { if (code !== 0) reject(new Error("task failed")); });
    });
}
// BENCH_END

try {
    console.log(`Ok(${await squareAsync(Number(process.argv[2]))})`);
} catch {
    console.log("Err(TaskError::Failed)");
}
