# Registry contract review for issue #88

This is a proposal for the next package-manager iteration. The current
[package contract](../spec/PACKAGE_REGISTRY.md) already supports portable
format-2 graph locks, an embedded registry, pinned Git packages, and an
offline content-addressed store. The
[measurement script](../scripts/measure_package_resolution.py) provides a
reproducible flat-graph benchmark without treating one host's timings as a
project-wide result.

## Suggested remote registry shape

1. Resolve an exact `name@version` from an immutable HTTPS index entry that
   supplies the package tree SHA-256 and source location. Resolve version
   constraints only while writing `tok.lock`; builds use locked exact versions.
2. Download source into staging, verify the existing tree digest, then promote
   it into the content-addressed store. An offline build succeeds only when
   every locked digest is already cached; an offline miss gives a distinct
   diagnostic without modifying the lockfile.
3. Keep source identity (`registry:name@version`) separate from download URL.
   A URL change must not silently change the package selected by a lockfile.
4. Document index trust separately from content integrity. A digest detects
   content changes after resolution; it does not establish who was allowed to
   publish a version. Signing, publisher identity, revocation, and vulnerability
   metadata need their own policy before accepting third-party uploads.

## Decisions required before implementation

- Who operates the index and which domain is authoritative?
- Are package names global, owner-scoped, or both? Can a version ever be
  replaced or deleted?
- What are the size and source-file limits for remote packages?
- Which signing and incident-response policy is required for third-party
  packages?

Until those decisions are made, `tok add --git` provides remote distribution
with pinned commits and verified source digests without a hosted registry.
