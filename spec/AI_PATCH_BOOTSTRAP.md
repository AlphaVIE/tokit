# Experimental function patch protocol

`tok ai-patch-index file.tok` lists each function's name, byte span, and
SHA-256 hash of its `fn ...` declaration. The name is stable when unrelated
source is inserted before a function; the hash detects changes to the target.
For a public function, the preceding `pub` is outside the hashed span and is
preserved by replacement.

Pass a function name as the last argument, for example
`tok ai-patch-index file.tok answer`, to receive only that function in the
same versioned schema. Missing or ambiguous names fail with `P003`. This
avoids returning every declaration hash when the caller already knows its
target.

`tok ai-patch entry.tok request.json` prints the proposed target file without
writing it. Add `--write` to save it. A version 1 request has this shape:

```json
{
  "version": 1,
  "target": "math.tok",
  "edits": [
    {
      "function": "answer",
      "expected_sha256": "<hash from ai-patch-index>",
      "replacement": "fn answer()->i32{7}"
    }
  ]
}
```

The target is a `.tok` file inside the entry directory and must belong to its
loaded module graph. Pinned package sources are excluded. Each replacement
contains exactly one function with the same name and no `pub` or surrounding
comments. All hashes and replacements are checked against one
source snapshot. The compiler then checks the entire loaded module graph with
the proposed text, including importers, before writing any change. `--write`
stages a file in the target directory and renames it over the original after
rechecking the snapshot. A failed request leaves the original source intact.
Errors are JSON on stderr; successful writes return JSON on stdout.

This first protocol replaces whole functions in one file. It does not yet
identify expression nodes, rename declarations, patch multiple files in one
transaction, or reconcile concurrent writers between the final read and
rename. Those are required for a general AST patch channel. The patch request
is ordinary text suitable for version control review.
