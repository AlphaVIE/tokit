# Experimental language server

`tok lsp` starts a language server on standard input and output. Configure an
editor's LSP client to launch the built `tok` executable with the `lsp`
argument for `.tok` files. The server uses JSON-RPC framing with UTF-8 body
byte lengths and returns UTF-16 positions, as required by its advertised
capability. No editor extension is bundled yet.

The current server supports `initialize`, `shutdown`, `exit`, full-document
`didOpen`/`didChange`/`didClose` synchronization, published diagnostics, and
top-level function, record, and enum document symbols, go-to-definition, and hover. It checks unsaved
buffers in memory, including relative imports and pinned package sources.
Diagnostics currently contain the first loader, parser, or checker error.
Changing an imported buffer rechecks dependent open documents. Closing a
document discards its unsaved text and clears stale diagnostics. Stale
document versions are ignored.

The server rechecks all open documents after each change. It has no
incremental dependency cache yet, and package lockfiles are still validated
against the saved package tree. Definitions resolve within the open document: a name goes to the nearest
preceding local binding in its function (parameter, `let`/`var`, `for`,
lambda parameter, or match binding) and otherwise to the top-level function,
record, or enum of that name. This is a lexical approximation, not full
scope resolution; names imported from other files return no location yet.
Hover shows the checked type of the expression under the cursor when the
document type-checks on its own, and otherwise the signature or declaration
of a top-level function, record, or enum.

Completion, cross-file definitions, rename,
incremental text edits, and editor-specific integration are future work.
This is an IDE bootstrap, not a full language-service implementation.

The protocol behavior follows the [Language Server Protocol specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/).
