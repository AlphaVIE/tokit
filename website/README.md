# Website

Tokit is a static site built from `src/` into `public/`. There are no frontend
package dependencies, trackers, remote fonts, or third-party scripts. The page
uses the local SVG icon, CSS, and a small inline script for the install tabs and
copy buttons.

The code examples live in `src/snippets/`. `build.py` type-checks them with
`tok check` and highlights them with `tok tokens`. CI rebuilds `public/` and
checks that the committed output is current.

## Build and preview locally

From the repository root:

```sh
cargo build --locked -p tokit-compiler
python3 website/build.py --tok target/debug/tok
python3 -m http.server 8765 --bind 127.0.0.1 --directory website/public
```

Open `http://127.0.0.1:8765/` locally. This preview only listens on the
machine's loopback address. For the release build, use
`cargo build --locked --release -p tokit-compiler` and pass
`target/release/tok` to `build.py`.

## GitHub Pages

`.github/workflows/pages.yml` builds and deploys `website/public` when website
files reach `main`, or when manually dispatched. To enable it, choose **GitHub
Actions** as the Pages build source in the repository's Pages settings. This
workflow does not run until it is pushed to GitHub.

The site uses relative asset links so it works at the default project URL
(`AlphaVIE.github.io/tokit/`) and under a custom domain. Once the desired
domain is known, set it as the custom domain in GitHub Pages settings and
configure DNS with your registrar. For a subdomain, use a CNAME record pointing
to `AlphaVIE.github.io`; for an apex domain, use the GitHub Pages A/AAAA
records shown in GitHub's current documentation. Enable **Enforce HTTPS** once
GitHub has issued the certificate. No domain is hardcoded in this draft.
