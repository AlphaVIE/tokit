# Source-aware compiler locations

`Span` carries a `SourceId` plus byte offsets local to that source. `parse_in_source` and `lex_in_source` let the file loader assign distinct IDs to separately read files. The default single-file `parse` and `lex` APIs use `SourceId(0)`, preserving current command behavior and diagnostic JSON fields. The checked expression-type map distinguishes equal byte offsets from different sources.

`SourceMap` registers each path and text under a loader-assigned ID. Diagnostics can render against the registered source and include its path in text or JSON. The native emitter also uses this map to calculate the correct line and column for checked expressions from multiple sources. Its original single-source API remains available.

The loader assigns IDs in depth-first import order, beginning with the entry file at zero. It canonicalizes paths, resolves relative `.tok` files inside the entry directory, loads a file once in a diamond graph, and rejects cycles and escapes. The CLI uses this graph for checking, running, testing, explaining, indexing, statistics, and native builds. The loaded program currently has one flat declaration namespace. Generated native runtime diagnostics include the correct line and column but do not yet print the source path; that and scoped module semantics remain follow-up work.
