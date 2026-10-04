package main

import (
    "fmt"
    "os"
    "strconv"
)

func walk(n int, next []int) int {
    index := 0
    total := 0
    for step := 0; step < n; step++ {
        index = next[index]
        total += index
    }
    return total
}

func main() {
    if len(os.Args) < 4 {
        panic("invalid arguments")
    }
    n, err := strconv.Atoi(os.Args[1])
    if err != nil {
        panic(err)
    }
    next := make([]int, len(os.Args)-2)
    for i := range next {
        next[i], err = strconv.Atoi(os.Args[i+2])
        if err != nil {
            panic(err)
        }
    }
    fmt.Printf("Ok(%d)\n", walk(n, next))
}
