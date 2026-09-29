package main

import (
    "fmt"
    "os"
    "strconv"
)

// BENCH_START
type Pair[T any] struct { Left, Right T }
func flip[T any](p Pair[T]) Pair[T] { return Pair[T]{p.Right, p.Left} }
// BENCH_END

func main() {
    left, right := os.Args[2], os.Args[3]
    switch os.Args[1] {
    case "i32":
        a, err := strconv.ParseInt(left, 10, 32); if err != nil { panic(err) }
        b, err := strconv.ParseInt(right, 10, 32); if err != nil { panic(err) }
        p := flip(Pair[int32]{int32(a), int32(b)})
        fmt.Printf("Pair(left:%d,right:%d)\n", p.Left, p.Right)
    case "bool":
        a, err := strconv.ParseBool(left); if err != nil { panic(err) }
        b, err := strconv.ParseBool(right); if err != nil { panic(err) }
        p := flip(Pair[bool]{a, b})
        fmt.Printf("Pair(left:%t,right:%t)\n", p.Left, p.Right)
    case "String":
        p := flip(Pair[string]{left, right})
        fmt.Printf("Pair(left:%q,right:%q)\n", p.Left, p.Right)
    default: panic("unknown type")
    }
}
