# Experimental filesystem-read capability

The current subset exposes two built-in functions:

```text
read_text(path:String) -> Result<String,IoError>
lines(text:String) -> [String]
```

`IoError` is a reserved built-in enum with `Denied`, `NotFound`, `InvalidUtf8`, and `Other` variants. `read_text` decodes the entire file as UTF-8 and returns an error value rather than a runtime diagnostic. `lines` follows Rust `str.lines()` behavior: it splits on LF, removes a CR immediately before LF, and does not add an empty final element for a trailing LF. Counting lines with `i32` arithmetic still reports `E201` on overflow.

Filesystem reads require an explicit grant. Without one, `read_text` returns `Err(IoError::Denied)`. The interpreter accepts one root with `tok run --allow-read <path> file.tok`. A binary made with `tok build` accepts the same `--allow-read <path>` flag when launched. The root may be a file or directory. Both runtimes canonicalize the root and requested path, then require the requested path to be the root or inside it. This resolves existing symlinks before the containment check. A missing requested file returns `NotFound` only when its existing parent is inside the grant; otherwise it returns `Denied`. A nonexistent grant grants nothing.

Program arguments can follow the source or binary after `--`, for example `tok run --allow-read data reader.tok -- data/input.txt` or `./reader --allow-read data -- data/input.txt`. The built-in `args()` sees only `data/input.txt` in these examples.

For example, from the repository root:

```text
tok run --allow-read examples examples/line_count.tok
# Ok(3)
tok build examples/line_count.tok -o line_count
./line_count --allow-read examples
# Ok(3)
```

This is a language-level gate in an experimental runtime, **not an operating-system sandbox**. The generated native process can still inherit ordinary OS privileges. The design has no write grant, multiple roots, streaming I/O, file metadata, stable filesystem ABI, or defense against a file being replaced between path resolution and reading. Future capability syntax and enforcement need a security review before they become language commitments.
