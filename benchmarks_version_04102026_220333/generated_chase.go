package main

import (
    "fmt"
    "os"
    "strconv"
)

func walk(n, size, stride int) int {
    next := make([]int, 0, size)
    for i := 0; i < size; i++ {
        value := i + stride
        if value >= size {
            value -= size
        }
        next = append(next, value)
    }
    index, total := 0, 0
    half := size / 2
    for step := 0; step < n; step++ {
        index = next[index]
        if index < half {
            total++
        }
    }
    return total
}

func main() {
    if len(os.Args) != 4 {
        panic("invalid arguments")
    }
    n, err := strconv.Atoi(os.Args[1])
    if err != nil { panic(err) }
    size, err := strconv.Atoi(os.Args[2])
    if err != nil { panic(err) }
    stride, err := strconv.Atoi(os.Args[3])
    if err != nil { panic(err) }
    fmt.Printf("Ok(%d)\n", walk(n, size, stride))
}
