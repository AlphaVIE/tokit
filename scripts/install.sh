#!/bin/sh
# Install the latest (or a given) Tokit release on Linux or macOS:
#   curl -sSf https://raw.githubusercontent.com/AlphaVIE/tokit/main/scripts/install.sh | sh
#   curl -sSf ... | sh -s -- v0.1.0
# Installs tok into $TOK_INSTALL (default ~/.tok/bin) after verifying its SHA-256.
set -eu

repo="AlphaVIE/tokit"
version="${1:-latest}"
dest="${TOK_INSTALL:-$HOME/.tok/bin}"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64 | Linux-arm64) target="aarch64-unknown-linux-gnu" ;;
  Darwin-arm64) target="aarch64-apple-darwin" ;;
  Darwin-x86_64) target="x86_64-apple-darwin" ;;
  *) echo "unsupported platform: $(uname -s) $(uname -m)" >&2; exit 1 ;;
esac

if [ "$version" = latest ]; then
  base="https://github.com/$repo/releases/latest/download"
else
  base="https://github.com/$repo/releases/download/$version"
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
archive="tok-$target.tar.gz"
curl -sSfL "$base/$archive" -o "$work/$archive"
curl -sSfL "$base/SHA256SUMS" -o "$work/SHA256SUMS"
expected="$(grep " $archive\$" "$work/SHA256SUMS" | cut -d' ' -f1)"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$work/$archive" | cut -d' ' -f1)"
else
  actual="$(shasum -a 256 "$work/$archive" | cut -d' ' -f1)"
fi
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
  echo "checksum mismatch for $archive" >&2
  exit 1
fi
tar xzf "$work/$archive" -C "$work"
mkdir -p "$dest"
cp "$work/tok-$target/tok" "$dest/tok"
chmod +x "$dest/tok"
echo "installed $("$dest/tok" --help >/dev/null 2>&1 && echo tok) to $dest/tok"
case ":$PATH:" in
  *":$dest:"*) ;;
  *) echo "add $dest to your PATH, for example: export PATH=\"$dest:\$PATH\"" ;;
esac
echo "tok run works now; tok build and tok run --native also need Rust (https://rustup.rs)."
