import { spawn } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  checkRegistryBundle,
  materializeRegistryBundle,
  stableStringify
} from "action-parity/src/generator.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const output = path.join(root, "apps", "redline-desktop", "src", "generated");
const bundlePath = path.join(output, "registry-bundle.json");
const checkMode = process.argv.includes("--check");

const bundle = await exportBundle();
if (checkMode) {
  const result = await checkRegistryBundle(bundle, output, { typescript: true });
  const expectedBundle = `${stableStringify(bundle, 2)}\n`;
  let bundleStatus = "missing";
  try {
    bundleStatus = (await readFile(bundlePath, "utf8")) === expectedBundle ? "current" : "drifted";
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
  result.files.push({ path: bundlePath, status: bundleStatus });
  result.ok = result.ok && bundleStatus === "current";
  for (const file of result.files) process.stdout.write(`${file.status}\t${file.path}\n`);
  process.exitCode = result.ok ? 0 : 1;
} else {
  await materializeRegistryBundle(bundle, output, { typescript: true });
  await writeFile(bundlePath, `${stableStringify(bundle, 2)}\n`, "utf8");
  process.stdout.write(`Generated Redline ActionParity artifacts in ${output}\n`);
}

function exportBundle() {
  return new Promise((resolve, reject) => {
    const child = spawn(
      "cargo",
      ["run", "--quiet", "-p", "redline-core", "--example", "export_action_registry"],
      { cwd: root, shell: false, windowsHide: true, stdio: ["ignore", "pipe", "inherit"] }
    );
    let stdout = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(`Redline Registry exporter exited with ${code}.`));
        return;
      }
      try {
        resolve(JSON.parse(stdout));
      } catch (error) {
        reject(new Error(`Redline Registry exporter returned invalid JSON: ${error.message}`));
      }
    });
  });
}
