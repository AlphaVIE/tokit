# Experimental JSON value and rendering module

The checked Tokit module [json.tok](../examples/json/json.tok) defines a
recursive value tree with Null, Bool(bool), Number(String), Text(String),
Array([Value]), and Object([Member]). A member stores a string key and a
value. Arrays and objects preserve source order; duplicate object keys
remain distinct.

The render function returns Result<String,RenderError> and produces compact
JSON. Number(String) stores the exact decimal lexeme, including fractions
and exponents. Rendering validates its grammar first and returns
Err(RenderError::InvalidNumber) for an empty, malformed, or non-ASCII
number. The module does not round numbers through i32 or floating point.
This preserves values that Tokit cannot yet compute numerically.

Strings are UTF-8. Rendering escapes quote, backslash, every control byte
below U+0020, and uses the short JSON escapes for backspace, tab, newline,
form feed, and carriage return. Other Unicode characters remain UTF-8.
Rendering nested arrays and objects is recursive and inherits the
prototype's 32-call depth guard. [main.tok](../examples/json/main.tok)
demonstrates an object; [tests.tok](../examples/json/tests.tok) exercises
numbers, escapes, and nesting.

The parse function returns Result<Value,JsonParseError>. Its error offset is
a zero-based UTF-8 byte position in the input. It accepts JSON scalars,
arrays, objects, the four JSON whitespace bytes, number lexemes, short
string escapes, and four-digit Unicode escapes. Valid UTF-16 surrogate
pairs become one Unicode scalar; lone surrogates are rejected because
Tokit strings require valid UTF-8. The parser preserves number lexemes,
object member order, and duplicate names. [parse_main.tok](../examples/json/parse_main.tok)
demonstrates a parse/render round trip, while
[parse_cli.tok](../examples/json/parse_cli.tok) accepts a JSON document as
one program argument. The accepted nesting depth is at most twelve
arrays/objects; deeper inputs return a typed error. This is an explicit
resource limit, not a claim that deeper JSON is invalid. The Unicode
choice excludes ill-formed strings allowed by the ABNF but identified as
non-interoperable in [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259.html).

This is a library experiment, not a packaged standard library. The
[local package example](../examples/package_json/main.tok) consumes this
module through a content-pinned `pkg:` import. The current implementation
copies packed `Bytes` values between parser functions and
uses packed mutable byte buffers for parsed numbers, strings, and rendered
string escaping. Array and object output uses mutable string append;
individual member text and escape pieces still use concatenation. Performance
evaluation and a packaged interface remain part of [the JSON issue](https://github.com/AlphaVIE/tokit/issues/83);
no linear-time or low-allocation claim is made.
