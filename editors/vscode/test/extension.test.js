const assert = require("node:assert/strict");
const { test } = require("node:test");
const Module = require("node:module");

test("server restarts are serialized and use the configured executable", async () => {
  let serverPath = "tok";
  let restartCommand;
  let configListener;
  let active = 0;
  let maxActive = 0;
  const commands = [];
  const vscode = {
    workspace: {
      getConfiguration: () => ({ get: () => serverPath }),
      onDidChangeConfiguration: (listener) => {
        configListener = listener;
        return { dispose() {} };
      },
    },
    commands: {
      registerCommand: (_name, command) => {
        restartCommand = command;
        return { dispose() {} };
      },
    },
    window: { showErrorMessage: (message) => assert.fail(message) },
  };
  class LanguageClient {
    constructor(_id, _name, serverOptions) {
      this.serverOptions = serverOptions;
    }
    async start() {
      commands.push(this.serverOptions.run.command);
      active++;
      maxActive = Math.max(maxActive, active);
    }
    async stop() {
      await new Promise((resolve) => setTimeout(resolve, 5));
      active--;
    }
  }

  const originalLoad = Module._load;
  Module._load = function (request, parent, isMain) {
    if (request === "vscode") return vscode;
    if (request === "vscode-languageclient/node") return { LanguageClient };
    return originalLoad.call(this, request, parent, isMain);
  };
  let extension;
  try {
    extension = require("../extension");
  } finally {
    Module._load = originalLoad;
  }

  await extension.activate({ subscriptions: [] });
  assert.deepEqual(commands, ["tok"]);
  serverPath = "/custom/bin/tok";
  configListener({ affectsConfiguration: (key) => key === "tokit.serverPath" });
  await Promise.all([restartCommand(), restartCommand()]);
  assert.equal(commands.at(-1), "/custom/bin/tok");
  assert.equal(maxActive, 1);
  assert.equal(active, 1);
  await extension.deactivate();
  assert.equal(active, 0);
});
