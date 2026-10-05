package main

import (
    "fmt"
    "os"
    "strconv"
)

func sumValues(n int, values []int64) int64 {
    index := 0
    var total int64
    for step := 0; step < n; step++ {
        total += values[index]
        index++
        if index == len(values) {
            index = 0
        }
    }
    return total
}

func main() {
    if len(os.Args) < 4 {
        panic("invalid arguments")
    }
    n, err := strconv.Atoi(os.Args[1])
    if err != nil { panic(err) }
    values := make([]int64, len(os.Args)-2)
    for i := range values {
        values[i], err = strconv.ParseInt(os.Args[i+2], 10, 64)
        if err != nil { panic(err) }
    }
    fmt.Printf("Ok(%d)\n", sumValues(n, values))
}
