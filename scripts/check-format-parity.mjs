#!/usr/bin/env node
/**
 * 格式注册表漂移检查 —— 影核协议第 15 条的「查绑定清单」。
 *
 * 唯一真相源是 Rust 的 `crates/redline-core/src/format.rs`；渲染层
 * `packages/redline-core/src/viewers/registry.ts` 只是它的一份投影。
 * 这个脚本把两边摆在一起比，对不上就退出码 1。
 *
 * 这不是形式主义：这两份表**已经漂过一次**——TS 把 .doc 当 docx 渲染，
 * Rust 明确拒绝 .doc；TS 认 psd/dxf，Python 那份不认。漂移不会自己报错，
 * 只会在某个用户打开一个 .doc 看到乱码时才暴露。
 *
 * 用法：
 *   node scripts/check-format-parity.mjs            # 用 target/debug/redline
 *   node scripts/check-format-parity.mjs --json     # 机器可读
 *   REDLINE_BIN=path/to/redline node scripts/...    # 指定二进制
 */
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const asJson = process.argv.includes("--json");

function fail(message, details) {
  if (asJson) {
    console.log(JSON.stringify({ ok: false, error: message, details }, null, 2));
  } else {
    console.error(`格式表漂移检查失败：${message}`);
    if (details) console.error(JSON.stringify(details, null, 2));
  }
  process.exit(1);
}

/** 从 Rust 核心拿权威格式表。 */
function loadRust() {
  const candidates = () => [
      process.env.REDLINE_BIN,
      join(root, "target", "debug", process.platform === "win32" ? "redline-cli.exe" : "redline-cli"),
      join(root, "target", "debug", process.platform === "win32" ? "redline.exe" : "redline"),
      join(root, "target", "release", process.platform === "win32" ? "redline-cli.exe" : "redline-cli"),
      join(root, "target", "release", process.platform === "win32" ? "redline.exe" : "redline"),
    ].filter(Boolean);
  let bin = candidates().find((candidate) => existsSync(candidate));
  if (!bin && !process.env.REDLINE_BIN) {
    execFileSync("cargo", ["build", "--quiet", "-p", "redline-cli"], {
      cwd: root,
      stdio: "inherit",
      windowsHide: true,
    });
    bin = candidates().find((candidate) => existsSync(candidate));
  }
  if (!bin) {
    fail("找不到 redline 二进制（可用 REDLINE_BIN 指定）", { tried: candidates() });
  }
  const raw = execFileSync(bin, ["formats", "--json"], { encoding: "utf-8" });
  const envelope = JSON.parse(raw);
  if (!envelope.ok) fail("核心返回了错误信封", envelope.error);
  return envelope.result;
}

/**
 * 从 TS 注册表源码里抠出映射表。
 *
 * 刻意用正则读源码而不是 import：这个脚本要能在没装依赖、没编译过的干净仓库里跑，
 * 而 registry.ts 一 import 就会拖进 react 和一堆 viewer。
 */
function loadTypescript() {
  const file = join(root, "packages", "redline-core", "src", "viewers", "registry.ts");
  const source = readFileSync(file, "utf-8");

  const block = (name) => {
    const start = source.indexOf(name);
    if (start === -1) fail(`registry.ts 里找不到 ${name}`);
    const open = source.indexOf("{", start);
    const close = source.indexOf("};", open);
    return source.slice(open, close);
  };

  const extToFormat = {};
  for (const [, ext, format] of block("const EXT_TO_FORMAT").matchAll(
    /["']?([\w-]+)["']?\s*:\s*"([\w-]+)"/g,
  )) {
    extToFormat[ext] = format;
  }

  const viewers = {};
  for (const [, format, viewer] of block("const LOADERS").matchAll(
    /["']?([\w-]+)["']?\s*:\s*\(\)\s*=>\s*import\("\.\/(\w+)"\)/g,
  )) {
    viewers[format] = viewer;
  }

  const refused = Object.keys(
    Object.fromEntries(
      [...block("export const REFUSED_EXTENSIONS").matchAll(/^\s*(\w+):\s*"/gm)].map((m) => [m[1], true]),
    ),
  );

  return { extToFormat, viewers, refused };
}

const rust = loadRust();
const ts = loadTypescript();
const problems = [];

// 1. 每个 Rust 格式的每个扩展名，TS 都要映射到同一个格式 id
for (const spec of rust.formats) {
  for (const ext of spec.extensions) {
    const mapped = ts.extToFormat[ext];
    if (mapped === undefined) {
      problems.push({ kind: "missing_extension", extension: ext, expected: spec.id });
    } else if (mapped !== spec.id) {
      problems.push({ kind: "format_mismatch", extension: ext, rust: spec.id, ts: mapped });
    }
  }
  // 2. viewer 绑定要一致 —— 「这个格式该用哪个组件渲染」也是核心说了算
  if (ts.viewers[spec.id] === undefined) {
    problems.push({ kind: "missing_viewer", format: spec.id, expected: spec.viewer });
  } else if (ts.viewers[spec.id] !== spec.viewer) {
    problems.push({ kind: "viewer_mismatch", format: spec.id, rust: spec.viewer, ts: ts.viewers[spec.id] });
  }
}

// 3. TS 不能凭空多认扩展名
const rustExtensions = new Set(rust.formats.flatMap((s) => s.extensions));
for (const ext of Object.keys(ts.extToFormat)) {
  if (!rustExtensions.has(ext)) {
    problems.push({ kind: "extra_extension", extension: ext, ts: ts.extToFormat[ext] });
  }
}

// 4. 被拒绝的老格式两边都要拒绝，且绝不能出现在支持表里
for (const { extension } of rust.refused) {
  if (ts.extToFormat[extension]) {
    problems.push({ kind: "refused_but_mapped", extension, ts: ts.extToFormat[extension] });
  }
  if (!ts.refused.includes(extension)) {
    problems.push({ kind: "refused_not_declared", extension });
  }
}

if (problems.length > 0) {
  fail(`两份格式表有 ${problems.length} 处对不上`, problems);
}

const summary = {
  ok: true,
  formats: rust.formats.length,
  extensions: rustExtensions.size,
  refused: rust.refused.length,
};
console.log(asJson ? JSON.stringify(summary, null, 2) : `格式表一致：${summary.formats} 种格式 / ${summary.extensions} 个扩展名 / ${summary.refused} 个明确拒绝`);
