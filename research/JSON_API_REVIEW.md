# JSON package API review for issue #83

This is a proposal for the experimental package, not a frozen language API.

## Current contract

- `Value` distinguishes null, boolean, exact number text, decoded text,
  arrays, and ordered object members. Keeping numbers as text preserves input
  lexemes such as `-0.25E+08` without an `f64` round trip.
- `parse(String)` returns `Result<Value, JsonParseError>` with a byte offset.
  Parsing rejects malformed UTF-8 escapes and nesting deeper than 256.
- `render(Value)` returns `Result<String, RenderError>`. It rejects invalid
  number text and emits object members in their stored order, including
  duplicate keys.
- The package is pure Tokit source. Input bytes use shared `Bytes` buffers,
  while composite `Value` copies and recursive rendering may still be costly.

## Proposed decisions

1. Keep exact number lexemes and ordered members. Do not silently convert
   numbers to floating point or collapse duplicate keys.
2. Keep byte offsets for parser errors. Add line and column only as a derived
   presentation layer, so one source of truth remains.
3. Keep the 256 nesting limit documented and test its boundary on both
   backends before treating it as a stable public promise.
4. Measure allocations and larger documents before changing ownership or
   introducing streaming. The current wall-clock reports include startup and
   rendering, so they do not identify the source of copies.

The expanded corpus in `compiler/tests/json_module.rs` compares pretty input
with canonical output from `serde_json`, including wide arrays and Unicode.
Owner review is still needed for the public package interface and ownership
tradeoffs before closing #83.
