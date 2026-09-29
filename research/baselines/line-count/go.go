package main

import (
    "bytes"
    "errors"
    "fmt"
    "os"
)

// BENCH_START
func lineCount(path string) (int32, error) {
    data, err := os.ReadFile(path)
    if err != nil { return 0, err }
    count := bytes.Count(data, []byte{'\n'})
    if len(data) > 0 && data[len(data)-1] != '\n' { count++ }
    return int32(count), nil
}
// BENCH_END

func main() {
    count, err := lineCount(os.Args[1])
    if errors.Is(err, os.ErrNotExist) { fmt.Println("Err(IoError::NotFound)"); return }
    if err != nil { panic(err) }
    fmt.Printf("Ok(%d)\n", count)
}
