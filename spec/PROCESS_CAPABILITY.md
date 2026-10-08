# Process capability

```text
struct Process{status:I,stdout:String,stderr:String}
exec(program:String,args:[String],stdin:String) -> Result<Process,IoError>
```

`exec` starts `program` with `args`, writes `stdin` to its standard input,
waits for it to finish, and returns its exit status and captured output. It
is how Tokit programs use other tools — `git`, `sqlite3`, `ffmpeg`, a Python
script, or another Tokit program — without a foreign function interface:
the other program runs in its own process, so it cannot corrupt Tokit's
memory or bypass its type system.

- The program is started directly, never through a shell, so arguments are
  passed verbatim and need no quoting; shell syntax such as pipes or `*` has
  no special meaning. `program` is looked up on `PATH` like any command.
- It needs the `--allow-run <program>` grant on `tok run` or a built binary;
  the grant must equal the `program` string exactly, and `--allow-run '*'`
  allows every program. Without a matching grant `exec` returns
  `Err(IoError::Denied)` before starting anything. A program that cannot be
  found yields `IoError::NotFound`; output that is not UTF-8 yields
  `IoError::InvalidUtf8`; other start failures yield `IoError::Other`.
- `status` is the exit code, or `-1` when the process ended without one (for
  example killed by a signal). A non-zero status is still `Ok`: the program
  ran.
- Each output stream is captured up to 16 MiB; the rest is discarded.
  Standard input is written concurrently with reading, so large inputs and
  outputs cannot deadlock.
- `exec` is an effect (`process.run` in `tok explain`) that spawned pure
  functions cannot call (`E117`). `Process` is a reserved built-in record.
- WebAssembly modules cannot start processes; `exec` returns
  `IoError::Other` there.

The child inherits the environment and working directory of the Tokit
program. A granted program can do anything the user can, so grant specific
programs rather than `*` for code you have not reviewed.
