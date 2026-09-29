package main

import (
    "fmt"
    "os"
    "strconv"
)

// BENCH_START
type DivError int
const DivZero DivError = 1
type Result struct { Value int32; Error DivError; Ok bool }
func divide(a, b int32) Result {
    if b == 0 { return Result{Error: DivZero} }
    return Result{Value: a / b, Ok: true}
}
// BENCH_END

func main() {
    a, err := strconv.ParseInt(os.Args[1], 10, 32)
    if err != nil { panic(err) }
    b, err := strconv.ParseInt(os.Args[2], 10, 32)
    if err != nil { panic(err) }
    result := divide(int32(a), int32(b))
    if result.Ok { fmt.Printf("Ok(%d)\n", result.Value) } else { fmt.Println("Err(DivError::DivZero)") }
}
