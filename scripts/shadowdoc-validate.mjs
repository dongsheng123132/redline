#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

const SHA256 = /^[a-f0-9]{64}$/;
const UNIT_ID = /^([a-z][a-z0-9_-]*):([1-9][0-9]*)$/;
const FORMAT_GRANULARITY = Object.freeze({
  docx: "paragraph",
  xlsx: "sheet",
  "pptx-outline": "slide",
  pdf: "page",
  text: "document",
  html: "document",
  archive: "entry",
  "archive-external": "entry",
});

function sha256(value) {
  return createHash("sha256").update(value, "utf8").digest("hex");
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function validateShadowDoc(document, file = "<memory>") {
  const violations = [];
  const report = (code, path, message) => violations.push({ code, path, message });

  if (!isRecord(document)) {
    report("invalid_root", "$", "根值必须是 JSON object");
    return result(file, violations);
  }

  if ("ok" in document && document.ok !== true) {
    report("action_failed", "$.ok", "这里只接受成功的 document.inspect 输出");
  }
  if ("action_id" in document && document.action_id !== "document.inspect") {
    report("wrong_action", "$.action_id", "动作信封必须来自 document.inspect");
  }

  const shadow = document.shadow;
  if (!isRecord(shadow)) {
    report("missing_shadow", "$.shadow", "缺少 ShadowDoc 描述对象");
  } else {
    if (shadow.schema !== "redline.shadow-document") {
      report("schema_mismatch", "$.shadow.schema", "schema 必须是 redline.shadow-document");
    }
    if (shadow.schemaVersion !== 1) {
      report("version_mismatch", "$.shadow.schemaVersion", "Core 0.1 只接受 schemaVersion 1");
    }
    if (typeof shadow.producer !== "string" || shadow.producer.length === 0) {
      report("invalid_producer", "$.shadow.producer", "producer 必须是非空字符串");
    }
    if (shadow.profile !== "semantic") {
      report("profile_mismatch", "$.shadow.profile", "Core 0.1 profile 必须是 semantic");
    }
    if (typeof shadow.lossy !== "boolean") {
      report("invalid_lossy", "$.shadow.lossy", "lossy 必须是 boolean");
    }
    if (!SHA256.test(shadow.sourceSha256 ?? "")) {
      report("invalid_source_hash", "$.shadow.sourceSha256", "必须是 64 位小写 SHA-256");
    }
  }

  const format = document.format;
  const expectedGranularity = FORMAT_GRANULARITY[format];
  if (!expectedGranularity) {
    report("unsupported_format", "$.format", "Core 0.1 不认识这个 format");
  } else if (isRecord(shadow) && shadow.granularity !== expectedGranularity) {
    report(
      "granularity_mismatch",
      "$.shadow.granularity",
      `${format} 的 granularity 必须是 ${expectedGranularity}`,
    );
  }

  const source = document.source;
  if (!isRecord(source)) {
    report("missing_source", "$.source", "缺少 source 对象");
  } else {
    if (typeof source.path !== "string" || source.path.length === 0) {
      report("invalid_source_path", "$.source.path", "source.path 必须是非空字符串");
    }
    if (typeof source.extension !== "string") {
      report("invalid_extension", "$.source.extension", "source.extension 必须是字符串");
    }
    if (!Number.isSafeInteger(source.bytes) || source.bytes < 0) {
      report("invalid_source_bytes", "$.source.bytes", "source.bytes 必须是非负安全整数");
    }
    if (!SHA256.test(source.sha256 ?? "")) {
      report("invalid_source_hash", "$.source.sha256", "必须是 64 位小写 SHA-256");
    }
    if (
      isRecord(shadow) &&
      SHA256.test(shadow.sourceSha256 ?? "") &&
      SHA256.test(source.sha256 ?? "") &&
      shadow.sourceSha256 !== source.sha256
    ) {
      report("source_hash_mismatch", "$.shadow.sourceSha256", "影文档没有绑定当前 source.sha256");
    }
  }

  if (!isRecord(document.summary)) {
    report("invalid_summary", "$.summary", "summary 必须是 JSON object");
  }

  if (!Array.isArray(document.units)) {
    report("invalid_units", "$.units", "units 必须是数组");
  } else {
    const seen = new Set();
    document.units.forEach((unit, index) => {
      const base = `$.units[${index}]`;
      if (!isRecord(unit)) {
        report("invalid_unit", base, "unit 必须是 object");
        return;
      }

      const idMatch = UNIT_ID.exec(unit.id ?? "");
      if (!idMatch) {
        report("invalid_unit_id", `${base}.id`, "id 必须是 <kind>:<正整数>");
      } else {
        if (seen.has(unit.id)) {
          report("duplicate_unit_id", `${base}.id`, `unit id ${unit.id} 重复`);
        }
        seen.add(unit.id);
        if (unit.kind !== idMatch[1]) {
          report("unit_kind_mismatch", `${base}.kind`, "kind 必须等于 id 的前缀");
        }
      }

      if (typeof unit.kind !== "string") {
        report("invalid_unit_kind", `${base}.kind`, "kind 必须是字符串");
      } else if (expectedGranularity && unit.kind !== expectedGranularity) {
        report(
          "unit_granularity_mismatch",
          `${base}.kind`,
          `当前 format 的 unit kind 必须是 ${expectedGranularity}`,
        );
      }
      if (typeof unit.label !== "string") {
        report("invalid_unit_label", `${base}.label`, "label 必须是字符串");
      }
      if (typeof unit.text !== "string") {
        report("invalid_unit_text", `${base}.text`, "text 必须是字符串");
      }
      if (!SHA256.test(unit.textSha256 ?? "")) {
        report("invalid_unit_hash", `${base}.textSha256`, "必须是 64 位小写 SHA-256");
      } else if (typeof unit.text === "string" && unit.textSha256 !== sha256(unit.text)) {
        report("unit_hash_mismatch", `${base}.textSha256`, "textSha256 与 text 的 UTF-8 SHA-256 不一致");
      }
      if ("note" in unit && typeof unit.note !== "string") {
        report("invalid_unit_note", `${base}.note`, "note 必须是字符串");
      }
    });
  }

  return result(file, violations);
}

function result(file, violations) {
  return {
    ok: violations.length === 0,
    spec: "shadowdoc-core-0.1",
    profile: "semantic",
    file,
    violations,
  };
}

function printUsage() {
  process.stderr.write("用法: node scripts/shadowdoc-validate.mjs FILE [--json]\n");
}

async function main(argv) {
  const json = argv.includes("--json");
  const positional = argv.filter((arg) => arg !== "--json");
  if (positional.length !== 1) {
    printUsage();
    process.exitCode = 64;
    return;
  }

  const file = positional[0];
  let value;
  try {
    value = JSON.parse(await readFile(file, "utf8"));
  } catch (error) {
    const output = result(file, [
      { code: "invalid_json", path: "$", message: error instanceof Error ? error.message : String(error) },
    ]);
    process.stdout.write(`${JSON.stringify(output)}\n`);
    process.exitCode = 1;
    return;
  }

  const output = validateShadowDoc(value, file);
  if (json || !output.ok) {
    process.stdout.write(`${JSON.stringify(output, null, json ? 2 : 0)}\n`);
  } else {
    process.stdout.write(`PASS ${file} (${value.units.length} units)\n`);
  }
  process.exitCode = output.ok ? 0 : 1;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  await main(process.argv.slice(2));
}
