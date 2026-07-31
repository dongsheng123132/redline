#!/usr/bin/env node

import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { access, readFile } from "node:fs/promises";
import { constants } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const MAX_OUTPUT_BYTES = 64 * 1024 * 1024;

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function exists(file) {
  try {
    await access(file, constants.R_OK);
    return true;
  } catch {
    return false;
  }
}

export function scoreRequiredText(text, requiredText = []) {
  const found = requiredText.map((expected) => ({ text: expected, found: text.includes(expected) }));
  return {
    found,
    recallPercent: requiredText.length === 0
      ? null
      : Math.round((found.filter((item) => item.found).length / requiredText.length) * 10_000) / 100,
  };
}

export function countTextBlocks(text) {
  if (text.length === 0) return 0;
  return text.split(/\r?\n[ \t]*\r?\n+/).filter((block) => block.trim().length > 0).length;
}

export function countDoclingItems(document) {
  const refs = new Set();
  const visit = (value) => {
    if (Array.isArray(value)) {
      value.forEach(visit);
      return;
    }
    if (!value || typeof value !== "object") return;
    if (typeof value.self_ref === "string" && typeof value.label === "string") {
      refs.add(value.self_ref);
    }
    Object.values(value).forEach(visit);
  };
  visit(document);
  return refs.size;
}

export function summarizeAttempts(attempts) {
  if (attempts.length === 0) throw new Error("attempts 不能为空");
  if (attempts.every((attempt) => attempt.status === "skipped")) return attempts[0];

  const successful = attempts.filter((attempt) => attempt.status === "ok");
  if (successful.length === 0) {
    return {
      ...attempts[0],
      repetitions: attempts.length,
      successes: 0,
    };
  }

  const durations = successful.map((attempt) => attempt.durationMs).sort((a, b) => a - b);
  const sample = successful[0];
  const { durationMs: _durationMs, ...stable } = sample;
  return {
    ...stable,
    status: successful.length === attempts.length ? "ok" : "partial",
    repetitions: attempts.length,
    successes: successful.length,
    durationMedianMs: durations[Math.floor(durations.length / 2)],
    durationMinMs: durations[0],
    durationMaxMs: durations.at(-1),
    outputDeterministic: new Set(successful.map((attempt) => attempt.outputTextSha256)).size === 1,
    failures: attempts
      .filter((attempt) => attempt.status !== "ok")
      .map(({ error, timedOut }) => ({ error, timedOut })),
  };
}

async function repeatAdapter(runOnce, repetitions) {
  const attempts = [];
  for (let index = 0; index < repetitions; index += 1) {
    attempts.push(await runOnce());
    if (attempts[0].status === "skipped") break;
  }
  return summarizeAttempts(attempts);
}

function findDoclingSourceHash(document) {
  const origin = document?.origin;
  if (!origin || typeof origin !== "object") return null;
  const candidate = origin.binary_hash ?? origin.binaryHash ?? null;
  return typeof candidate === "string" ? candidate.toLowerCase() : null;
}

async function runProcess(command, args, timeoutMs) {
  const started = process.hrtime.bigint();
  return await new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: ROOT,
      shell: false,
      windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    let outputBytes = 0;
    let settled = false;
    let timedOut = false;

    const finish = (result) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      const durationMs = Number(process.hrtime.bigint() - started) / 1_000_000;
      resolve({
        ...result,
        durationMs: Math.round(durationMs * 1000) / 1000,
        stdout: Buffer.concat(stdout).toString("utf8"),
        stderr: Buffer.concat(stderr).toString("utf8"),
        timedOut,
      });
    };
    const collect = (chunks, chunk) => {
      outputBytes += chunk.length;
      if (outputBytes > MAX_OUTPUT_BYTES) {
        child.kill();
        finish({ ok: false, code: null, error: "output_limit_exceeded" });
        return;
      }
      chunks.push(chunk);
    };
    child.stdout.on("data", (chunk) => collect(stdout, chunk));
    child.stderr.on("data", (chunk) => collect(stderr, chunk));
    child.on("error", (error) => finish({ ok: false, code: null, error: error.message }));
    child.on("close", (code) => finish({ ok: code === 0, code, error: code === 0 ? null : "nonzero_exit" }));
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill();
    }, timeoutMs);
  });
}

async function pythonPackageVersion(python, packageName, timeoutMs) {
  const script = "import importlib.metadata,sys; print(importlib.metadata.version(sys.argv[1]))";
  const run = await runProcess(python, ["-c", script, packageName], timeoutMs);
  return run.ok ? run.stdout.trim() : null;
}

async function detectRedline(explicit) {
  const candidates = [
    explicit,
    process.env.REDLINE_CLI,
    path.join(ROOT, "target", "release", process.platform === "win32" ? "redline-cli.exe" : "redline-cli"),
    path.join(ROOT, "target", "debug", process.platform === "win32" ? "redline-cli.exe" : "redline-cli"),
  ].filter(Boolean);
  for (const candidate of candidates) {
    const absolute = path.resolve(candidate);
    if (await exists(absolute)) return absolute;
  }
  return null;
}

async function runRedline(command, file, sourceHash, requiredText, timeoutMs) {
  if (!command) return { adapter: "redline", status: "skipped", reason: "redline_cli_not_found" };
  const run = await runProcess(command, ["--json", "inspect", file], timeoutMs);
  if (!run.ok) return failed("redline", run);
  try {
    const output = JSON.parse(run.stdout);
    if (output.ok !== true) return failed("redline", { ...run, error: output.error?.code ?? "inspect_failed" });
    const text = output.units.map((unit) => unit.text ?? "").join("\n\n");
    return measured({
      adapter: "redline",
      version: output.tool ?? null,
      run,
      text,
      units: output.units.length,
      sourceHashBound: output.shadow?.sourceSha256 === sourceHash && output.source?.sha256 === sourceHash,
      requiredText,
    });
  } catch (error) {
    return failed("redline", { ...run, error: `invalid_json: ${error.message}` });
  }
}

async function runMarkItDown(python, version, file, requiredText, timeoutMs) {
  if (!version) return { adapter: "markitdown", status: "skipped", reason: "python_package_not_installed" };
  const script = [
    "from markitdown import MarkItDown",
    "import sys",
    "result=MarkItDown(enable_plugins=False).convert_local(sys.argv[1])",
    "print(result.text_content,end='')",
  ].join(";");
  const run = await runProcess(python, ["-c", script, file], timeoutMs);
  if (!run.ok) return failed("markitdown", run, version);
  return measured({
    adapter: "markitdown",
    version,
    run,
    text: run.stdout,
    units: countTextBlocks(run.stdout),
    sourceHashBound: false,
    requiredText,
  });
}

async function runDocling(python, version, file, sourceHash, requiredText, timeoutMs) {
  if (!version) return { adapter: "docling", status: "skipped", reason: "python_package_not_installed" };
  const script = [
    "from docling.document_converter import DocumentConverter",
    "import json,sys",
    "doc=DocumentConverter().convert(sys.argv[1]).document",
    "print(json.dumps({'text':doc.export_to_markdown(),'document':doc.export_to_dict()},ensure_ascii=False))",
  ].join(";");
  const run = await runProcess(python, ["-c", script, file], timeoutMs);
  if (!run.ok) return failed("docling", run, version);
  try {
    const output = JSON.parse(run.stdout);
    const document = output.document;
    return measured({
      adapter: "docling",
      version,
      run,
      text: output.text ?? "",
      units: countDoclingItems(document),
      sourceHashBound: findDoclingSourceHash(document) === sourceHash,
      requiredText,
    });
  } catch (error) {
    return failed("docling", { ...run, error: `invalid_json: ${error.message}` }, version);
  }
}

function measured({ adapter, version, run, text, units, sourceHashBound, requiredText }) {
  const score = scoreRequiredText(text, requiredText);
  return {
    adapter,
    status: "ok",
    version,
    durationMs: run.durationMs,
    outputTextBytes: Buffer.byteLength(text, "utf8"),
    outputTextChars: [...text].length,
    outputTextSha256: sha256(Buffer.from(text, "utf8")),
    structuralUnits: units,
    sourceHashBound,
    requiredTextRecallPercent: score.recallPercent,
    requiredText: score.found,
  };
}

function failed(adapter, run, version = null) {
  return {
    adapter,
    status: "failed",
    version,
    durationMs: run.durationMs,
    timedOut: run.timedOut,
    error: run.error,
    stderrTail: run.stderr?.slice(-2000) ?? "",
  };
}

async function main(argv) {
  const options = {};
  let manifestArg = null;
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (["--redline", "--python", "--markitdown-python", "--docling-python"].includes(arg)) {
      const value = argv[index + 1];
      if (!value) {
        process.stderr.write(`${arg} 缺少路径\n`);
        process.exitCode = 64;
        return;
      }
      options[arg.slice(2)] = value;
      index += 1;
    } else if (arg.startsWith("--")) {
      process.stderr.write(`未知参数: ${arg}\n`);
      process.exitCode = 64;
      return;
    } else if (!manifestArg) {
      manifestArg = arg;
    } else {
      process.stderr.write(`多余参数: ${arg}\n`);
      process.exitCode = 64;
      return;
    }
  }
  if (!manifestArg) {
    process.stderr.write(
      "用法: node scripts/shadowdoc-pk.mjs MANIFEST [--redline PATH] [--python PATH] [--markitdown-python PATH] [--docling-python PATH]\n",
    );
    process.exitCode = 64;
    return;
  }

  const manifestPath = path.resolve(manifestArg);
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  if (manifest.schema !== "shadowdoc-pk-corpus/0.1" || !Array.isArray(manifest.cases)) {
    throw new Error("manifest 必须是 shadowdoc-pk-corpus/0.1 且包含 cases 数组");
  }

  const timeoutMs = (manifest.timeoutSeconds ?? 180) * 1000;
  const repetitions = manifest.repetitions ?? 3;
  if (!Number.isSafeInteger(repetitions) || repetitions < 1 || repetitions > 20) {
    throw new Error("manifest.repetitions 必须是 1 到 20 的整数");
  }
  const python = options.python ?? process.env.SHADOWDOC_PYTHON ?? "python";
  const markitdownPython =
    options["markitdown-python"] ?? process.env.SHADOWDOC_MARKITDOWN_PYTHON ?? python;
  const doclingPython =
    options["docling-python"] ?? process.env.SHADOWDOC_DOCLING_PYTHON ?? python;
  const redline = await detectRedline(options.redline ?? null);
  const [markitdownVersion, doclingVersion] = await Promise.all([
    pythonPackageVersion(markitdownPython, "markitdown", 10_000),
    pythonPackageVersion(doclingPython, "docling", 10_000),
  ]);

  const cases = [];
  for (const item of manifest.cases) {
    const file = path.resolve(path.dirname(manifestPath), item.path);
    if (!(await exists(file))) {
      cases.push({ id: item.id, file, status: "skipped", reason: "corpus_file_not_found" });
      continue;
    }
    const sourceHash = sha256(await readFile(file));
    const requiredText = Array.isArray(item.requiredText) ? item.requiredText : [];
    const adapters = await Promise.all([
      repeatAdapter(
        () => runRedline(redline, file, sourceHash, requiredText, timeoutMs),
        repetitions,
      ),
      repeatAdapter(
        () => runMarkItDown(markitdownPython, markitdownVersion, file, requiredText, timeoutMs),
        repetitions,
      ),
      repeatAdapter(
        () => runDocling(doclingPython, doclingVersion, file, sourceHash, requiredText, timeoutMs),
        repetitions,
      ),
    ]);
    cases.push({ id: item.id, format: item.format, file, sourceSha256: sourceHash, adapters });
  }

  process.stdout.write(`${JSON.stringify({
    schema: "shadowdoc-pk-report/0.1",
    manifest: manifestPath,
    fairness: {
      sameLocalSourceBytes: true,
      requiredTextMatch: "case-sensitive exact substring",
      sourceHashBound: "adapter output explicitly carries the SHA-256 of the same source bytes",
      caveat: "duration includes process startup; run on a warm machine and repeat before publishing performance claims",
    },
    repetitions,
    environment: {
      platform: process.platform,
      arch: process.arch,
      node: process.version,
      python,
      markitdownPython,
      doclingPython,
      redline,
    },
    installed: { markitdown: markitdownVersion, docling: doclingVersion },
    cases,
  }, null, 2)}\n`);
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  await main(process.argv.slice(2));
}
