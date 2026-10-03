package main

import (
	"fmt"
	"os"
	"strconv"
)

func arraySum(n int, values []int) int {
    index := 0
    total := 0
    for i := 0; i < n; i++ {
        total += values[index]
        if index == len(values)-1 {
            index = 0
        } else {
            index++
        }
    }
    return total
}

func main() {
    if len(os.Args) != 5 {
        panic("invalid arguments")
    }
    n, err := strconv.Atoi(os.Args[1])
    if err != nil {
        panic(err)
    }
    values := make([]int, 3)
    for i := range values {
        values[i], err = strconv.Atoi(os.Args[i+2])
        if err != nil {
            panic(err)
        }
    }
    fmt.Printf("Ok(%d)\n", arraySum(n, values))
}
