# Experimental AI index

`tok ai-index file.tok` checks the entry file and its imports and prints one compact JSON
object. If checking fails, it prints the normal diagnostic on stderr and no
index. This command does not run user code.

The single-file schema has `version: 1` and source-order `records`, `enums`, and
`functions`. Records include generic parameters and ordered fields. Enums
include variants and optional payload types. Functions include their byte
`span`, type parameters, parameter and return types, sorted direct `calls`,
`builtins`, record `constructs`, `direct_effects`, and transitive `effects`.
The call and effect sets are derived from the checked AST; recursion is handled
to a fixed point. Byte spans identify the current source snapshot and are not
stable AST identities.

When imports load multiple files, the index uses `version: 2`. A `sources`
array contains canonical file paths in loader order, and each declaration span
becomes `[source_id,start,end]`. Source IDs are indexes into `sources`.

Effect labels currently mean:

| Label | Meaning |
| --- | --- |
| `fs.read` | Calls `read_text`; execution needs a path grant. |
| `fs.write` | Calls `write_text`; execution needs a separate write grant. |
| `env.args` | Reads program arguments with `args`. |
| `task.spawn` | Starts a task with `spawn`. |
| `task.join` | Joins a task with `join`. |

The index summarizes possible calls, including branches that might not run.
It does not claim that an effect occurs on every execution. It does not contain
exports, data-flow proofs, ownership analysis, stable node IDs, or a project
dependency graph. These effect labels do not enumerate arithmetic or bounds
failures, allocation, or local mutation. Those require later compiler and
language work.
