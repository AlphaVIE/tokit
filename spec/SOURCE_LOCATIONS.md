# Source-aware compiler locations

`Span` carries a `SourceId` plus byte offsets local to that source. `parse_in_source` and `lex_in_source` let a future module loader assign distinct IDs to separately read files. The default single-file `parse` and `lex` APIs use `SourceId(0)`, preserving current command behavior and diagnostic JSON fields. The checked expression-type map can now distinguish equal byte offsets from different sources.

This is an internal foundation, not module loading. The current CLI still reads one file, and diagnostic display still receives one source string. Before enabling imports, the compiler needs a source registry for paths and text, source-aware diagnostic rendering, and a verified multi-file program loader. IDs should be assigned deterministically from the resolved import graph, rather than from process-global state.
