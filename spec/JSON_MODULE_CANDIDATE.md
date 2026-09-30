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

This is a library experiment, not a packaged standard library. The current
file loader only imports paths inside the entry file's directory, so examples
keep a copy of the module beside their entry file. Parsing is the next stage
of [the JSON issue](https://github.com/AlphaVIE/tokit/issues/83).
The present implementation copies arrays and concatenates strings during
rendering; no linear-time or low-allocation performance claim is made.
