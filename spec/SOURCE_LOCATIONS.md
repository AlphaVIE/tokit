# Source-aware compiler locations

`Span` carries a `SourceId` plus byte offsets local to that source. `parse_in_source` and `lex_in_source` let a future module loader assign distinct IDs to separately read files. The default single-file `parse` and `lex` APIs use `SourceId(0)`, preserving current command behavior and diagnostic JSON fields. The checked expression-type map can now distinguish equal byte offsets from different sources.

`SourceMap` registers each path and text under a loader-assigned ID. Diagnostics can render against the registered source and include its path in text or JSON. The native emitter also uses this map to calculate the correct line and column for checked expressions from multiple sources. Its original single-source API remains available.

This is an internal foundation, not module loading. The current CLI still reads one file. Before enabling imports, the compiler needs a verified multi-file program loader and path-aware runtime diagnostics in generated binaries. IDs should be assigned deterministically from the resolved import graph, rather than from process-global state.
