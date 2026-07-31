/**
 * 前端通向动作核心的**唯一一扇门**。
 *
 * 界面里任何一个按钮，最终都要走 `call(actionId, params)`。这条规矩不是洁癖：
 * 一旦有人图方便在前端「就地算一下」（自己判扩展名、自己拼 diff、自己决定能不能
 * 覆盖文件），GUI 就成了第二份实现，行为迟早跟 CLI 和 MCP 分岔。
 *
 * 对应的后端在 `src-tauri/src/lib.rs` 的 `redline_call`，它也只是原样转发给
 * `redline_core::dispatch` —— 跟 CLI 的 `redline call` 是同一个入口。
 */
import { invoke } from "@tauri-apps/api/core";
import type { RedlineFormat, ShadowDescriptor } from "redline-core";

/** 核心的统一输出信封。成功失败同一个形状，只有 ok 位不同。 */
export type Envelope<T = Record<string, unknown>> =
  | ({ ok: true; version: number; tool: string; action_id: string } & T)
  | {
      ok: false;
      version: number;
      tool: string;
      action_id: string;
      error: {
        class: "input" | "refused" | "internal";
        code: string;
        message: string;
        details?: unknown;
      };
    };

/** Action ID 常量。跟 Rust 的 `action::id` 一一对应，改名等于破坏兼容。 */
export const ACTION = {
  inspect: "document.inspect",
  diff: "document.diff",
  apply: "document.apply-track-changes",
  verify: "document.verify",
  formats: "document.formats",
  archiveList: "archive.list",
  archiveExtract: "archive.extract",
  agentCatalog: "agent.catalog",
  agentDispatch: "agent.dispatch",
} as const;

export async function call<T = Record<string, unknown>>(
  action: string,
  params: Record<string, unknown> = {},
): Promise<Envelope<T>> {
  return (await invoke("redline_call", { action, params })) as Envelope<T>;
}

/** 把信封里的错误转成一句可以直接显示给人的话。 */
export function errorText(envelope: Envelope<unknown>): string | null {
  if (envelope.ok) return null;
  const label =
    envelope.error.class === "refused"
      ? "已拒绝"
      : envelope.error.class === "input"
        ? "输入有问题"
        : "内部错误";
  return `${label}：${envelope.error.message}`;
}

// ---- 下面是各动作的返回结构，跟 Rust 侧的 serde 输出一一对应 ----

export interface Unit {
  id: string;
  kind: string;
  label: string;
  text: string;
  textSha256: string;
  note?: string;
}

export interface SourceInfo {
  path: string;
  extension: string;
  bytes: number;
  sha256: string;
}

export interface Snapshot {
  shadow: ShadowDescriptor;
  format: RedlineFormat;
  source: SourceInfo;
  summary: Record<string, unknown>;
  units: Unit[];
}

export interface DiffChange {
  id: string;
  label: string;
  before: string;
  after: string;
  kind: "added" | "removed" | "modified";
}

export interface DiffReport {
  format: string;
  before: SourceInfo;
  after: SourceInfo;
  summary: { totalUnits: number; changedUnits: number; unchangedUnits: number };
  changes: DiffChange[];
}

export interface AgentInfo {
  id: string;
  label: string;
  program: string;
  args: string[];
  permissionNote: string;
  installed: boolean;
  resolvedPath: string | null;
}

export interface DispatchReport {
  agent: string;
  label: string;
  command: string[];
  permissionNote: string;
  cwd: string;
  prompt: string;
  source: string;
  output: string;
  exitCode: number | null;
  timedOut: boolean;
  durationMs: number | null;
  stdoutTail: string | null;
  stderrTail: string | null;
  /** 「成没成」看这个，不看 exitCode —— agent 可以退 0 但什么都没写。 */
  outputExists: boolean;
}
