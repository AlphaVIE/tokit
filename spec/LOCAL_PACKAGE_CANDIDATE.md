# Experimental content-pinned local packages

An entry file can import a local `.tok` source or a local source directory
outside its own directory when the entry directory contains `tok.toml`:

```toml
[dependencies]
json = { path = "../json/json.tok", sha256 = "0481914ddcf03c76d9fac14a510e09f04ec028576b4ccf9faa9458e4b02d1f1b" }
```

```tok
import json="pkg:json";
```

For a source directory, declare an entry file and pin the whole `.tok` tree:

```toml
[dependencies]
calc = { path = "../lib", entry = "api.tok", sha256 = "00fd1e1fc160d2cd6010600a444a8dc8880bab3ad55e7c1f579992dc9efe9718" }
```

[The directory example](../examples/package_tree/app/main.tok) imports
`api.tok`, which imports `internal/math.tok` inside the pinned tree.

The path is relative to `tok.toml`, not to the importing source. The
`sha256` field is the lowercase SHA-256 digest of the exact source bytes;
`tok pkg-hash path/to/file.tok` or `tok pkg-hash path/to/directory` prints
it. `tok add <entry.tok> <name> <relative-path>` adds a local file dependency,
calculates its pin, and updates both `tok.toml` and `tok.lock`. For a package
directory use `tok add <entry.tok> <name> <relative-directory> --entry <relative.tok>`.
`tok rm <entry.tok> <name>` removes its declaration and updates the lockfile;
it does not delete package files or edit source imports. Paths use `/` and
are relative to the entry file's directory. Existing manifest comments are
preserved, and an existing dependency name is not overwritten. The commands
validate the resulting package set before writing and reject symlink manifest
or lockfile destinations. Manual edits remain valid: run `tok lock entry.tok`
after editing `tok.toml` to generate the required `tok.lock` beside the entry
file. A directory digest uses SHA-256 with the domain prefix
`tokit-package-tree-v1\0`, followed by each `.tok` file in sorted relative
path order. Each file contributes the big-endian 64-bit length of its
slash-separated UTF-8 relative path, the path bytes, the big-endian 64-bit
content length, and the exact content bytes. Other files are ignored;
symlinks anywhere in the tree are rejected. The loader checks all declared
sources, then parses those same verified byte snapshots. Changed source
content fails with `E120` until the manifest digest is updated. Sources
must be UTF-8. A single-file dependency cannot import other files; a
directory dependency can import relative `.tok` paths only within its
pinned tree. Package-to-package imports remain unsupported.

`tok.lock` format 1 records the compiler version, the `portable-source`
target class, the SHA-256 digest of the exact manifest bytes, and each
direct dependency's name, relative manifest path, optional entry file,
content digest, and sorted logical source paths. The loader compares the
file with a freshly computed canonical lock representation. A missing or
stale file reports `E121`; invalid manifests or source content report
`E120`. The lockfile contains no absolute checkout paths. The target
field describes source compatibility only; it does not lock a native
binary or host toolchain. See the checked-in [single-file lock](../examples/package_json/tok.lock)
and [module-tree lock](../examples/package_tree/app/tok.lock).

Imported declarations keep the same `pub` visibility rules as local file
modules. Source maps, diagnostics, AI index names, and generated source use
stable logical names such as `pkg/json.tok`, `pkg/calc/api.tok`, and
`pkg::json::parse`. A
package source cannot also be imported by a relative path. Ordinary
relative imports still cannot leave the entry directory. The package
path may leave it only when explicitly declared and pinned in the manifest.

[The JSON example](../examples/package_json/main.tok) consumes the
experimental JSON module from a separate directory. This is a candidate
for reviewing package identity and import syntax, not a stable package
manager. There is no registry, download, transitive package graph,
package version resolution, signature, or cache yet. Source pinning verifies
content but does not establish who published it. The design must be reviewed before expanding
to remote packages or freezing compatibility guarantees.
