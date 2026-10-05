# Experimental language server

`tok lsp` starts a language server on standard input and output. Configure an
editor's LSP client to launch the built `tok` executable with the `lsp`
argument for `.tok` files. The server uses JSON-RPC framing with UTF-8 body
byte lengths and returns UTF-16 positions, as required by its advertised
capability. No editor extension is bundled yet.

The current server supports `initialize`, `shutdown`, `exit`, full-document
`didOpen`/`didChange`/`didClose` synchronization, published diagnostics, and
top-level function, record, and enum document symbols. It checks unsaved
standalone buffers in memory. Diagnostics currently contain the first parser
or checker error. Closing a document clears its diagnostics. Stale document
versions are ignored.

Imported files receive syntax diagnostics and document symbols, but semantic
diagnostics are deferred until an in-memory module graph can resolve open
buffers and package imports together. Completion, definition lookup, rename,
incremental text edits, and editor-specific integration are future work. This
is an IDE bootstrap, not a full language-service implementation.

The protocol behavior follows the [Language Server Protocol specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/).
