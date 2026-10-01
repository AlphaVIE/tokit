# Experimental content-pinned local packages

An entry file can import one local `.tok` source outside its directory when
the entry directory contains `tok.toml`:

```toml
[dependencies]
json = { path = "../json/json.tok", sha256 = "6a89f202f1c9ddca87e07838691d9b63713761ba4c374e3c4ee159269c0997fc" }
```

```tok
import json="pkg:json";
```

The path is relative to `tok.toml`, not to the importing source. The
`sha256` field is the lowercase SHA-256 digest of the exact source bytes;
`tok pkg-hash path/to/file.tok` prints it. The loader reads and checks each
declared dependency once, then parses those same verified bytes. A changed
file fails with `E120` until the manifest digest is updated. A dependency
must be a UTF-8 `.tok` file, with no imports of its own. This makes the
current content pin cover the entire package implementation.

Imported declarations keep the same `pub` visibility rules as local file
modules. Source maps, diagnostics, AI index names, and generated source use
stable logical names such as `pkg/json.tok` and `pkg::json::parse`. A
package source cannot also be imported by a relative path. Ordinary
relative imports still cannot leave the entry directory. The package
path may leave it only when explicitly declared and pinned in the manifest.

[The JSON example](../examples/package_json/main.tok) consumes the
experimental JSON module from a separate directory. This is a candidate
for reviewing package identity and import syntax, not a stable package
manager. There is no registry, download, transitive package graph,
multi-file package, package version resolution, signature, cache, lockfile,
or `tok add` command yet. Source pinning verifies content but does not
establish who published it. The design must be reviewed before expanding
to remote packages or freezing compatibility guarantees.
