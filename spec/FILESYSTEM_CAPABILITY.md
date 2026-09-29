# Experimental filesystem capabilities

The current subset exposes three built-in functions:

```text
read_text(path:String) -> Result<String,IoError>
write_text(path:String,text:String) -> Result<Unit,IoError>
lines(text:String) -> [String]
```

`IoError` is a reserved built-in enum with `Denied`, `NotFound`, `InvalidUtf8`, and `Other` variants. `read_text` decodes the entire file as UTF-8 and returns an error value rather than a runtime diagnostic. `write_text` writes the UTF-8 bytes of its second argument, replacing an existing file or creating a new file when its parent directory exists, and returns `Ok(())`. It does not create directories. `lines` follows Rust `str.lines()` behavior: it splits on LF, removes a CR immediately before LF, and does not add an empty final element for a trailing LF. Counting lines with `i32` arithmetic still reports `E201` on overflow.

Reads and writes need **separate** explicit grants. Without the corresponding grant, each function returns `Err(IoError::Denied)`. The interpreter accepts `tok run [--allow-read <path>] [--allow-write <path>] file.tok`; the same flags work when launching a binary made with `tok build`. `tok test` accepts them too. Grant flags may appear in either order before the source or binary program arguments. Each root may be a file or directory. A nonexistent grant grants nothing.

Both runtimes canonicalize granted roots and existing requested paths, then require the resolved path to be the root or inside it. This resolves existing symlinks before the containment check. A missing read target returns `NotFound` only when its existing parent is inside the read grant; otherwise it returns `Denied`. To create a file, `write_text` resolves its existing parent directory and requires that parent inside the write grant. A dangling symlink is denied; an existing symlink into the grant writes its resolved target. A missing parent or path outside the grant is denied. Other OS write failures return `IoError::Other`.

Program arguments can follow the source or binary after `--`, for example `tok run --allow-read data reader.tok -- data/input.txt` or `./reader --allow-read data -- data/input.txt`. The built-in `args()` sees only `data/input.txt` in these examples.

For example, from the repository root:

```text
tok run --allow-read examples examples/line_count.tok
# Ok(3)
tok build examples/line_count.tok -o line_count
./line_count --allow-read examples
# Ok(3)

# After creating input/ and output/ and placing input/a.txt:
tok run --allow-read input --allow-write output examples/file_copy.tok -- input/a.txt output/a.txt
# Ok(())
```

This is a language-level gate in an experimental runtime, **not an operating-system sandbox**. The generated native process can still inherit ordinary OS privileges. The checks do not defend against a path or symlink being replaced between resolution and opening or writing; concurrent filesystem changes can defeat containment. Do not treat a grant as a security boundary against an adversary who can mutate the filesystem at the same time. The design has one root per operation, no streaming I/O, file metadata, stable filesystem ABI, or atomic write transaction. Future capability syntax and enforcement need a security review before they become language commitments.
