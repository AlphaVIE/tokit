# Tokit for Visual Studio Code

Syntax highlighting for `.tok` files and the `tok lsp` language server:
diagnostics, document symbols, go-to-definition, hover, references, rename,
and completion.

## Install from source

1. Build the compiler and put `tok` on your `PATH`, or note its full path:

   ```bash
   cargo build --locked --release -p tokit-compiler
   ```

2. Package and install the extension:

   ```bash
   cd editors/vscode
   npm ci
   npx --no-install vsce package
   cd ../..
   ```

   ```bash
   code --install-extension editors/vscode/tokit-0.1.0.vsix
   ```

3. If `tok` is not on your `PATH`, set **Tokit: Server Path**
   (`tokit.serverPath`) to the executable, for example
   `target/release/tok.exe`.

Run **Tokit: Restart Language Server** after rebuilding `tok`; changing the
server path restarts it automatically.

Other editors can use the same server: configure their LSP client to run
`tok lsp` for `.tok` files (see [the LSP contract](../../spec/LSP_BOOTSTRAP.md)).
