/**
 * `RedlineHost` 的 Tauri 实现 —— **宿主能力**，不是业务动作。
 *
 * 读字节、存标注、开外部程序，这三件事换个宿主（Electron / 浏览器 / MCP server）
 * 就要重写一遍；而业务动作（inspect/diff/apply/dispatch）永远只有 `core.ts` 那一份。
 * 这条界线就是 redline-core 能被任意宿主复用的原因。
 */
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";
import type { RedlineDocument, RedlineHost } from "redline-core";
import { ACTION, call, errorText, type Snapshot } from "./core";

export const tauriHost: RedlineHost = {
  async readFileBytes(path: string): Promise<ArrayBuffer> {
    const bytes = await invoke<number[]>("read_file_bytes", { path });
    return new Uint8Array(bytes).buffer;
  },

  async inspectDocument(path: string): Promise<RedlineDocument> {
    const envelope = await call<Snapshot>(ACTION.inspect, { path });
    if (!envelope.ok) {
      throw new Error(errorText(envelope) ?? "无法生成语义影子");
    }
    return {
      docId: envelope.source.sha256,
      sourcePath: envelope.source.path,
      sourceSha256: envelope.source.sha256,
      format: envelope.format,
      shadow: envelope.shadow,
      units: envelope.units.map((unit, index) => ({
        id: unit.id,
        kind: unit.kind,
        index,
        label: unit.label,
        text: unit.text,
        textSha256: unit.textSha256,
      })),
      notes: envelope.units.flatMap((unit) => (unit.note ? [unit.note] : [])),
    };
  },

  annotationStore: {
    async get(key: string) {
      return await invoke<string | null>("kv_get", { key });
    },
    async set(key: string, value: string | null) {
      await invoke("kv_set", { key, value });
    },
  },

  async openExternal(path: string) {
    await openPath(path);
  },

  getSrcUrl(path: string) {
    return convertFileSrc(path);
  },
};
