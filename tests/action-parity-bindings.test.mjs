import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import test, { after } from "node:test";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifest = JSON.parse(
  await readFile(path.join(root, "apps/redline-desktop/src/generated/action-parity.json"), "utf8"),
);
const observations = [];
const cliActions = actionsFor("cli");
const guiActions = actionsFor("gui");

after(async () => {
  const directory = path.join(root, "artifacts/action-parity");
  await mkdir(directory, { recursive: true });
  await writeFile(
    path.join(directory, "parity-observations.json"),
    `${JSON.stringify(observations, null, 2)}\n`,
    "utf8",
  );
});

test("generated contract describes the real Redline Surface subset", () => {
  assert.equal(manifest.actions.length, 9);
  assert.deepEqual(cliActions, manifest.actions.map((action) => action.id).sort());
  assert.deepEqual(guiActions, [
    "agent.catalog",
    "agent.dispatch",
    "document.diff",
    "document.inspect",
  ]);
});

test("all nine CLI bindings reach the executable Registry", () => {
  const build = spawnSync("cargo", ["build", "--quiet", "-p", "redline-cli"], spawnOptions());
  assert.equal(build.status, 0, build.stderr);
  const binary = path.join(root, "target/debug", process.platform === "win32" ? "redline-cli.exe" : "redline-cli");

  for (const actionId of cliActions) {
    const executionId = `evidence-cli-${actionId}`;
    const args = [
      "--json",
      "--execution-id",
      executionId,
      "call",
      actionId,
      "--params",
      "{}",
      "--confirmed",
    ];
    const result = spawnSync(binary, args, spawnOptions());
    assert.notEqual(result.status, null, result.stderr);
    const envelope = JSON.parse(result.stdout);
    assert.equal(envelope.action_id, actionId);
    assert.equal(envelope.execution_id, executionId);
    observations.push({
      action_id: actionId,
      surface: "cli",
      request_execution_id: executionId,
      core_execution_id: envelope.execution_id,
    });
  }
});

test("all four GUI bindings reach the Registry through action-parity-tauri", () => {
  const result = spawnSync(
    "cargo",
    ["run", "--quiet", "-p", "redline-desktop", "--example", "export_gui_binding_observations"],
    spawnOptions(),
  );
  assert.equal(result.status, 0, result.stderr);
  const guiObservations = JSON.parse(result.stdout);
  assert.deepEqual(
    guiObservations.map((item) => item.action_id).sort(),
    guiActions,
  );
  for (const observation of guiObservations) {
    assert.equal(observation.request_execution_id, observation.core_execution_id);
    observations.push(observation);
  }
});

function actionsFor(surface) {
  return manifest.actions
    .filter((action) => action.bindings.some((binding) => binding.surface === surface))
    .map((action) => action.id)
    .sort();
}

function spawnOptions() {
  return {
    cwd: root,
    shell: false,
    windowsHide: true,
    encoding: "utf8",
    maxBuffer: 4 * 1024 * 1024,
  };
}
