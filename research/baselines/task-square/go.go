package main

import (
    "fmt"
    "os"
    "strconv"
)

// BENCH_START
func squareAsync(x int32) (int32, error) {
    result := make(chan int32, 1)
    go func() { result <- x * x }()
    return <-result, nil
}
// BENCH_END

func main() {
    value, err := strconv.ParseInt(os.Args[1], 10, 32)
    if err != nil { panic(err) }
    square, err := squareAsync(int32(value))
    if err != nil { fmt.Println("Err(TaskError::Failed)") } else { fmt.Printf("Ok(%d)\n", square) }
}
