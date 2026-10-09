// Starts `tok lsp` for .tok files and restarts it on demand or when the
// configured server path changes.

const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

let client;
let restartQueue = Promise.resolve();

function createClient() {
  const command = vscode.workspace.getConfiguration("tokit").get("serverPath", "tok");
  const server = { command, args: ["lsp"] };
  return new LanguageClient(
    "tokit",
    "Tokit",
    { run: server, debug: server },
    { documentSelector: [{ scheme: "file", language: "tokit" }, { scheme: "untitled", language: "tokit" }] },
  );
}

async function restartNow() {
  if (client) {
    await client.stop();
  }
  client = createClient();
  try {
    await client.start();
  } catch (error) {
    vscode.window.showErrorMessage(
      `Tokit: could not start "${client.serverOptions?.run?.command ?? "tok"} lsp" (${error.message}). Set tokit.serverPath.`,
    );
  }
}

function restart() {
  restartQueue = restartQueue.catch(() => {}).then(restartNow);
  return restartQueue;
}

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("tokit.restartServer", restart),
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("tokit.serverPath")) {
        restart();
      }
    }),
  );
  return restart();
}

function deactivate() {
  return restartQueue.catch(() => {}).then(() => client?.stop());
}

module.exports = { activate, deactivate };
