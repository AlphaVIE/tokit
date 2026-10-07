# Security model

What Tokit protects against, how, and where the protection ends.

## Language-level safety

Checked programs cannot read uninitialized or freed memory, index out of
bounds, overflow integers silently, dereference null, race on shared data, or
reinterpret bytes as another type ([type system](TYPE_SYSTEM.md),
[runtime model](RUNTIME_MODEL.md)). There is no `unsafe` escape hatch and no
FFI. These guarantees rest on the bootstrap implementation in Rust and on the
Rust code the native backend emits, which contains no `unsafe` blocks.

## Capabilities

Effects are grouped into capabilities. A program without grants can compute,
read its arguments, standard input, environment variables, and clocks, and
write standard output — nothing else.

| Capability | Builtins | Grant |
| --- | --- | --- |
| `fs.read` | `read_text`, `read_bytes`, `list_dir`, `exists` | `--allow-read <path>` |
| `fs.write` | `write_text`, `write_bytes`, `make_dir`, `remove_file` | `--allow-write <path>` |
| `net.connect` | `http_request`, `tcp_connect` | `--allow-net <host:port>` or `*` |
| `net.listen` | `serve`, `listen`, `accept` | `--allow-net <host:port>` or `*` |

A missing grant returns `Err(IoError::Denied)` before any system call, so
programs handle it like any other error. Grants apply to `tok run` and to
binaries built with `tok build` alike. Path grants are canonicalized and
checked against the resolved target ([filesystem
capability](FILESYSTEM_CAPABILITY.md)); network grants match the exact
`host:port` ([network capability](NETWORK_CAPABILITY.md)).

`tok explain file.tok` lists every effect a program can perform, transitively
through its imports, without running it — a reviewer or an AI agent can
compare that list against the grants it is about to give.

Pure code is enforced: `spawn` accepts only functions that perform no effect
at all (`E117`), so parallel tasks cannot leak data or race on resources.

## Supply chain

- Registry packages are embedded in the compiler and copied into a
  content-addressed store (`$TOK_HOME/store/<sha256>`); a corrupted store
  entry is detected and replaced ([package registry](PACKAGE_REGISTRY.md)).
- Path dependencies are pinned by SHA-256 in `tok.toml`, and `tok.lock`
  records the resolved graph; a changed dependency fails the build (`E121`)
  until it is re-locked.
- Packages are Tokit source, so they are checked like the program itself and
  their effects appear in `tok explain`. They cannot run code at install or
  build time.

## Limits

- Grants are a language-level gate, not an operating-system sandbox. The
  compiler, `rustc`, and the produced binary run with the user's privileges;
  run untrusted programs inside an OS sandbox or container.
- Denial of service is out of scope: a program may loop forever or allocate
  until memory runs out. HTTP servers cap heads at 64 KiB, bodies at 16 MiB,
  and idle reads at 30 seconds; WebSocket messages are capped at 16 MiB.
- There is no TLS. `http_request`, `serve`, and the Redis, PostgreSQL, and
  WebSocket packages speak plaintext; put a TLS-terminating proxy in front of
  services and use them on trusted networks only.
- `sha1` and `md5` exist for protocol compatibility only. `random_bytes` is
  OS-seeded and suitable for nonces and identifiers but is not a vetted
  CSPRNG for long-term keys. There is no constant-time comparison builtin.
- A remote package registry, package signatures, and SBOM output do not
  exist yet; today's registry is the set of packages reviewed into this
  repository.

## Reporting

Security issues should be reported privately to the repository owner rather
than in public issues.
