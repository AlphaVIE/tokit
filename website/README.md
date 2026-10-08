# Website

A static site: one HTML page, one stylesheet, an SVG icon, and a few lines of
inline JavaScript for the install tabs and copy buttons — about 23 KiB in
total, with no frameworks, web fonts, trackers, or build dependencies beyond
Python and `tok`.

- `src/` holds the page template, stylesheet, and assets.
- `src/snippets/*.tok` are the code samples. `build.py` type-checks each one
  with `tok check` and highlights it with Tokit's own lexer (`tok tokens`),
  so the page cannot show code that does not compile and ships no
  highlighter.
- `public/` is the generated site. CI rebuilds it and fails if it is stale.

```text
python website/build.py --tok target/release/tok
python -m http.server 8765 --directory website/public    # preview
```

## Hosting

Not deployed. Upload the contents of `public/` to any static host when the
site is ready.
