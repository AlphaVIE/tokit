import { readFileSync } from "node:fs";

// BENCH_START
function lineCount(path: string): number {
    const text = readFileSync(path, "utf8");
    return (text.match(/\n/g)?.length ?? 0) + Number(text.length > 0 && !text.endsWith("\n"));
}
// BENCH_END

try {
    console.log(`Ok(${lineCount(process.argv[2])})`);
} catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") console.log("Err(IoError::NotFound)");
    else throw error;
}
