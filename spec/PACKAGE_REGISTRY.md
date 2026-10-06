# Package registry

`tok` ships with the official package registry: the directories under
[`packages/`](../packages) are embedded into the compiler at build time, so
adding a package works offline and reproducibly.

```text
tok new app          # app/tok.toml, app/main.tok, app/tok.lock
cd app
tok search           # list registry packages: name@version  description
tok add json         # or tok add json@0.1.0
tok rm json
```

`tok add name` writes `name = { version = "0.1.0", sha256 = "..." }` to
`[dependencies]` and regenerates `tok.lock`; the code imports it with
`import json="pkg:json";`. The digest is the package tree digest of the
[local package contract](LOCAL_PACKAGE_CANDIDATE.md), covering the
package's `tok.toml` and every `.tok` source. Loading a registry dependency
requires that the registry contains exactly that name, version, and digest;
otherwise it reports `E120`. The lockfile records the package as
`path = "registry:name@version"` with its digest and sources, never a local
store path, so lockfiles are portable between machines.

Before loading, registry packages are materialized into a content-addressed
store at `$TOK_HOME/store/<sha256>/` (default `~/.tok/store`). An entry is
written to a staging directory, verified against its digest, and then
renamed into place; a missing, partial, or modified entry is rebuilt from
the embedded registry. All projects on a machine share the store.

The registry currently contains:

| Package | Purpose |
| --- | --- |
| `json` | Parse and render JSON values with byte-offset errors (nesting up to 256). |
| `http` | Routing with `:param` segments, 404/405, query and URL decoding, header lookup, `text`/`json`/`redirect` responses on top of `serve`. |
| `redis` | RESP2 client over `Conn`: `cmd`, `ping`, `auth`, `fetch`, `store`, `incr`, `delete`, nested replies. |
| `postgres` | PostgreSQL protocol v3: trust, cleartext, MD5, and SCRAM-SHA-256 authentication with server-signature check; `query`, parameterized `query_with`, `execute`, `quote`, `close`; text values with `NULL` as `None`. |

Each package is tested end to end in both backends: `http` serves a real
API, and `redis` and `postgres` talk to in-process servers that implement
their wire protocols (the PostgreSQL server verifies SCRAM proofs). They
have not yet been run against production Redis or PostgreSQL deployments in
CI; TLS (`sslmode=require`) is not supported.

Each package directory holds `tok.toml` with a `[package]` table (`name`,
`version`, `entry`, `description`) and a `[dependencies]` table, which may
name other registry packages in the same `version`/`sha256` form. Registry
and local path dependencies can be mixed in one project.

This registry is a curated, versioned set shipped with the compiler. Remote
registries, version ranges, publishing, signatures, SBOM output, and
vulnerability metadata are not implemented yet; the content digests already
make every resolved graph reproducible and tamper-evident.
