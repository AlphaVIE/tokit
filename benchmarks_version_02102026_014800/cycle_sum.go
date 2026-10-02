package main

import (
	"fmt"
	"os"
	"strconv"
)

func cycleSum(n int) int {
    value := 1
    total := 0
    for i := 0; i < n; i++ {
        total += value
        if value == 3 {
            value = 1
        } else {
            value++
        }
    }
    return total
}

func main() {
    if len(os.Args) != 2 {
        panic("invalid arguments")
    }
    n, err := strconv.Atoi(os.Args[1])
    if err != nil {
        panic(err)
    }
    fmt.Printf("Ok(%d)\n", cycleSum(n))
}
