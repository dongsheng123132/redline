#!/usr/bin/env node
/**
 * 核心纯净度检查 —— 把两条口头承诺变成机器门禁。
 *
 * 1. **`redline-core` 是无界面的。** 这是它能同时服务 GUI / CLI / MCP、
 *    能被别的项目 `cargo add` 直接嵌入的全部前提。一旦有人图方便在核心里
 *    引一个 tauri/wry，核心就再也不能在无头环境里跑了 —— 而且这种依赖
 *    加进去的时候什么都不会报错，等到别人 embed 失败才发现。
 *
 * 2. **不许出现 GPL/AGPL 依赖。** 项目要能被闭源商业产品放心嵌入，
 *    传染性协议是红线。这条以前只写在文档里，现在编进 CI。
 *
 * 用法：
 *   node scripts/check-core-purity.mjs
 *   node scripts/check-core-purity.mjs --json
 */
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const asJson = process.argv.includes("--json");

/**
 * 界面/窗口/浏览器内核相关的 crate。核心的**正常依赖**里出现任何一个就算污染。
 *
 * 注意这里刻意**不禁网络库**（reqwest/tokio 之类）：云端 agent 适配器将来要走
 * HTTP，那仍然是无界面的。要守的是「无界面」，不是「无依赖」。
 */
const GUI_CRATES = [
  "tauri", "tauri-build", "tauri-runtime", "tauri-runtime-wry", "tauri-utils",
  "wry", "tao", "winit", "muda", "softbuffer", "raw-window-handle",
  "gtk", "gdk", "gdk-pixbuf", "webkit2gtk", "javascriptcore-rs",
  "cocoa", "objc2-app-kit", "objc2-foundation",
  "iced", "egui", "eframe", "druid", "slint", "dioxus",
];

/** 传染性协议。所有可选项都命中才拒绝——项目要能被闭源产品嵌入。 */
const COPYLEFT = [/\bAGPL/i, /\bGPL-2/i, /\bGPL-3/i, /^GPL\b/i, /\bSSPL/i];
/** LGPL 之类：动态链接通常可接受，但值得人看一眼，只警告不拦。 */
const NEEDS_REVIEW = [/\bLGPL/i, /\bMPL-1/i, /\bCDDL/i];
/** 宽松协议。SPDX 表达式里只要有一个可选项是这些，就可以合规使用。 */
const PERMISSIVE = [/\bMIT\b/i, /\bApache-2\.0/i, /\bBSD-/i, /\bISC\b/i, /\bZlib\b/i, /\bUnlicense\b/i, /\bCC0-/i, /\bMPL-2\.0/i];

/**
 * 拆 SPDX 表达式的顶层可选项。
 *
 * **`OR` 意味着使用者可以任选一个**——`MIT OR GPL-3.0` 是完全可用的，
 * 按 MIT 用就行。不做这个区分会把一大批正常 crate（比如 r-efi 的
 * `MIT OR Apache-2.0 OR LGPL-2.1-or-later`）误报成传染性依赖，
 * 而误报多了之后这个检查就会被人无视 —— 那还不如没有。
 */
function licenseOptions(expression) {
  return expression
    .split(/\s+OR\s+/i)
    .map((part) => part.replace(/[()]/g, "").trim())
    .filter(Boolean);
}

/** 至少有一个可选项是纯宽松的（不含传染条款），就算合规。 */
function hasPermissiveOption(expression) {
  return licenseOptions(expression).some(
    (option) =>
      PERMISSIVE.some((re) => re.test(option)) &&
      !COPYLEFT.some((re) => re.test(option)) &&
      !NEEDS_REVIEW.some((re) => re.test(option)),
  );
}

function fail(message, details) {
  if (asJson) {
    console.log(JSON.stringify({ ok: false, error: message, details }, null, 2));
  } else {
    console.error(`核心纯净度检查失败：${message}`);
    if (details) console.error(JSON.stringify(details, null, 2));
  }
  process.exit(1);
}

let metadata;
try {
  metadata = JSON.parse(
    execFileSync("cargo", ["metadata", "--format-version", "1", "--all-features"], {
      cwd: root,
      encoding: "utf-8",
      maxBuffer: 64 * 1024 * 1024,
    }),
  );
} catch (err) {
  fail("cargo metadata 跑不起来", { message: String(err.message ?? err) });
}

const byId = new Map(metadata.packages.map((p) => [p.id, p]));
const nodes = new Map((metadata.resolve?.nodes ?? []).map((n) => [n.id, n]));

const corePackage = metadata.packages.find((p) => p.name === "redline-core");
if (!corePackage) fail("workspace 里找不到 redline-core");

/**
 * 从某个包出发，沿**正常依赖**（不含 dev / build）走全图。
 * dev-dependencies 里出现什么都无所谓——那不会进别人的构建。
 */
function normalDependencyClosure(startId) {
  const seen = new Set();
  const queue = [startId];
  while (queue.length > 0) {
    const id = queue.shift();
    if (seen.has(id)) continue;
    seen.add(id);
    for (const dep of nodes.get(id)?.deps ?? []) {
      const isNormal = (dep.dep_kinds ?? []).some((k) => k.kind === null || k.kind === undefined);
      if (isNormal && !seen.has(dep.pkg)) queue.push(dep.pkg);
    }
  }
  seen.delete(startId);
  return seen;
}

const problems = [];

// ---- 检查 1：核心不许沾界面 ----
const coreDeps = normalDependencyClosure(corePackage.id);
for (const id of coreDeps) {
  const pkg = byId.get(id);
  if (pkg && GUI_CRATES.includes(pkg.name)) {
    problems.push({
      kind: "core_not_headless",
      crate: pkg.name,
      version: pkg.version,
      hint: "redline-core 必须无界面。界面相关的东西放 apps/redline-desktop。",
    });
  }
}

// ---- 检查 2：全 workspace 不许有传染性协议 ----
const warnings = [];
for (const pkg of metadata.packages) {
  const license = pkg.license ?? "";
  // 本地 path 依赖（我们自己的 crate）跳过
  if ((metadata.workspace_members ?? []).includes(pkg.id)) continue;
  if (!license && !pkg.license_file) {
    warnings.push({ crate: pkg.name, version: pkg.version, license: "(未声明)" });
    continue;
  }
  // 有宽松可选项就直接通过，不管表达式里还写了什么
  if (hasPermissiveOption(license)) continue;

  if (COPYLEFT.some((re) => re.test(license))) {
    problems.push({ kind: "copyleft_dependency", crate: pkg.name, version: pkg.version, license });
  } else if (NEEDS_REVIEW.some((re) => re.test(license))) {
    warnings.push({ crate: pkg.name, version: pkg.version, license });
  }
}

if (problems.length > 0) {
  fail(`发现 ${problems.length} 处违规`, problems);
}

const summary = {
  ok: true,
  coreDependencies: coreDeps.size,
  totalPackages: metadata.packages.length,
  needsReview: warnings,
};

if (asJson) {
  console.log(JSON.stringify(summary, null, 2));
} else {
  console.log(
    `核心纯净：redline-core 正常依赖 ${coreDeps.size} 个包，无任何界面/窗口/浏览器内核依赖`,
  );
  console.log(`协议干净：${metadata.packages.length} 个包里没有 GPL/AGPL/SSPL`);
  if (warnings.length > 0) {
    // 只提示不拦：LGPL 动态链接通常可接受，但值得人看一眼
    console.log(`\n需要人工确认的 ${warnings.length} 个：`);
    for (const w of warnings) console.log(`  ${w.crate}@${w.version}  ${w.license}`);
  }
}
