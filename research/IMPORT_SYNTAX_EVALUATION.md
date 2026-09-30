# Experimental module syntax measurement

The first file-loading prototype used `import"math.tok";` and placed all
declarations in one namespace. That is short, but a second file declaring the
same private helper fails to check. The scoped prototype uses
`import m="math.tok";` and `pub` for cross-file visibility.

Locally installed `cl100k_base` and `o200k_base` gave this small spelling
sample. Counts are tokenizer-specific and do not measure generation or repair:

| Snippet | UTF-8 bytes | cl100k | o200k |
| --- | ---: | ---: | ---: |
| `import m="math.tok";` | 20 | 7 | 7 |
| `import"math.tok" as m;` | 22 | 8 | 9 |
| `import"math.tok";` | 17 | 5 | 6 |
| `pub fn triple(n:i32)->i32{n*3}` | 30 | 13 | 14 |
| `export fn triple(n:i32)->i32{n*3}` | 33 | 13 | 14 |

For one entry file with three calls, the explicit alias version
`import m="math.tok";fn main()->i32{m::triple(1)+m::triple(2)+m::triple(3)}`
measured 74 bytes and 34/34 tokens. A path-derived alias spelling with
`math::triple` measured 80 bytes and 32/33 tokens. The earlier flat form
measured 62 bytes and 26/27 tokens. These are not sufficient to choose a
canonical Tokit syntax. The current choice makes module resolution explicit
and permits duplicate private names.

A controlled repair comparison uses these edit counts as a proxy for repair
cost. They count source locations that must change, not model success rates:

| Change after initial generation | Flat import | Path-derived alias | Explicit alias |
| --- | ---: | ---: | ---: |
| Add a second file with the same private `helper` name, used once in each file | rename declaration and call in one file (2) | 0 | 0 |
| Rename `math.tok` to `arithmetic.tok` with three calls | change path only (1) | change path and three call prefixes (4) | change path only (1) |
| Expose a private function after `E119` | no visibility diagnostic | add `pub` at declaration (1) | add `pub` at declaration (1) |

The flat form has the smallest source-token count in the sample, but it cannot
express private imported names. Explicit aliases cost a few more tokens and
reduce edits when filenames change. These cases motivate the current
experimental spelling; complete multi-file generation and repair benchmarks
are still needed before a stable syntax decision.
