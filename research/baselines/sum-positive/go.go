package main

import (
    "fmt"
    "os"
    "strconv"
)

// BENCH_START
func sumPositive(xs []int32) int32 {
    var total int32
    for _, x := range xs {
        if x > 0 { total += x }
    }
    return total
}
// BENCH_END

func main() {
    xs := make([]int32, 0, len(os.Args)-1)
    for _, arg := range os.Args[1:] {
        n, err := strconv.ParseInt(arg, 10, 32)
        if err != nil { panic(err) }
        xs = append(xs, int32(n))
    }
    fmt.Println(sumPositive(xs))
}
