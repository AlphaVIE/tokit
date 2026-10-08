# tokit-lang.org

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

The `Website` workflow publishes `public/` to GitHub Pages on every change to
`website/` on `main`. For the custom domain:

1. In the repository settings, under Pages, set the source to "GitHub
   Actions" and the custom domain to `tokit-lang.org`, then enable "Enforce
   HTTPS" once the certificate is issued.
2. At the domain registrar, point the apex domain to GitHub Pages with `A`
   records `185.199.108.153`, `185.199.109.153`, `185.199.110.153`, and
   `185.199.111.153` (and `AAAA` records `2606:50c0:8000::153`,
   `2606:50c0:8001::153`, `2606:50c0:8002::153`, `2606:50c0:8003::153`), plus
   a `CNAME` record `www` → `alphavie.github.io`.

Any other static host works too: upload the contents of `public/`.
