# Interpreted array length benchmark

The program builds an `[i32]` of runtime-selected size, then adds its length
for a runtime-selected iteration count. The expected output is
`Ok(iterations * size)` for positive inputs that stay in the signed `i32`
range. A run with `2,000` iterations and `4,096` elements must print
`Ok(8192000)`; with `16,384` elements it must print `Ok(32768000)`.

This focused benchmark measures `tok run`, including interpreter startup,
parsing, checking, array construction, and the loop. It is intended for
before/after changes to the interpreter's array-length path. The source
uses runtime arguments, and the harness must validate each output against
the formula. Native execution is outside this benchmark's scope.
