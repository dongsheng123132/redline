/**
 * Redline 三栏工作台。
 *
 *   左：原文预览 + 标注（人在这儿圈）
 *   中：选哪个 AI 去改 + 派发 + 运行状态
 *   右：agent 产出的新文件预览 + 与原文的结构化差异
 *
 * 这个布局能成立，靠的是「Redline 从不写回源文件」这条纪律：右栏不是「改过的原文」，
 * 是**另一个文件**。所以左右两栏其实是同一个 RedlinePanel 开在两个路径上，
 * 不满意就继续在左栏加标注、再派发一次，源文件全程一个字节没动。
 */
import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { RedlinePanel, type RedlineAnnotation, type RedlineDocument } from "redline-core";
import { tauriHost } from "./host";
import {
  ACTION,
  call,
  errorText,
  type AgentInfo,
  type DiffReport,
  type DispatchReport,
} from "./core";

const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** 给一个源文件配一个产物路径：同目录、同扩展名、加 `-redline` 后缀。 */
function suggestOutput(source: string): string {
  const dot = source.lastIndexOf(".");
  const slash = Math.max(source.lastIndexOf("/"), source.lastIndexOf("\\"));
  if (dot <= slash) return `${source}-redline`;
  return `${source.slice(0, dot)}-redline${source.slice(dot)}`;
}

export default function App() {
  const [source, setSource] = useState<string | null>(null);
  const [output, setOutput] = useState<string>("");
  const [annotations, setAnnotations] = useState<RedlineAnnotation[]>([]);
  const [doc, setDoc] = useState<RedlineDocument | null>(null);

  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [agentId, setAgentId] = useState<string>("");
  const [instruction, setInstruction] = useState("");

  const [running, setRunning] = useState(false);
  const [report, setReport] = useState<DispatchReport | null>(null);
  const [diff, setDiff] = useState<DiffReport | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  /** 产物版本号：改一次强制右栏重新读文件，否则 agent 覆盖同一路径时看到的还是旧内容。 */
  const [outputVersion, setOutputVersion] = useState(0);

  useEffect(() => {
    void (async () => {
      const envelope = await call<{ agents: AgentInfo[] }>(ACTION.agentCatalog);
      if (!envelope.ok) return setMessage(errorText(envelope));
      setAgents(envelope.agents);
      setAgentId(envelope.agents.find((a) => a.installed)?.id ?? envelope.agents[0]?.id ?? "");
    })();
  }, []);

  // 文件关联/命令行带进来的文件：双击一个 zip 就该直接看见内容，
  // 而不是看见一个还要再点「打开文件…」的空工作台。
  useEffect(() => {
    void (async () => {
      const path = await invoke<string | null>("initial_file");
      if (!path) return;
      setSource(path);
      setOutput(suggestOutput(path));
    })();
  }, []);

  const pickFile = useCallback(async () => {
    const picked = await open({ multiple: false, title: "打开任意文件" });
    if (typeof picked !== "string") return;
    setSource(picked);
    setOutput(suggestOutput(picked));
    setReport(null);
    setDiff(null);
    setMessage(null);
  }, []);

  const handleAnnotations = useCallback((next: RedlineAnnotation[], nextDoc: RedlineDocument | null) => {
    setAnnotations(next);
    setDoc(nextDoc);
  }, []);

  /** 标注 → 核心要的格式。只给 unitId + note，正文由核心从快照补，界面不重复读文档。 */
  const payloadAnnotations = useMemo(
    () =>
      annotations.map((a) => ({
        unitId: a.unitId,
        note: a.note?.trim() || `${a.kind === "note" ? "批注" : "圈选"}位置需要修改`,
      })),
    [annotations],
  );

  const selectedAgent = agents.find((a) => a.id === agentId);

  const dispatchToAgent = useCallback(async () => {
    if (!source || !agentId) return;
    setRunning(true);
    setMessage(null);
    setReport(null);
    try {
      const envelope = await call<DispatchReport>(ACTION.agentDispatch, {
        agent: agentId,
        source,
        output,
        annotations: payloadAnnotations,
        instruction,
      });
      if (!envelope.ok) {
        setMessage(errorText(envelope));
        return;
      }
      setReport(envelope);
      setOutputVersion((v) => v + 1);

      // 产物出来了才去做对比。没产物就别拿一个不存在的文件去 diff，那只会报一个
      // 令人困惑的「文件不存在」，掩盖掉真正的问题（agent 没写出来）。
      if (envelope.outputExists) {
        const compared = await call<DiffReport>(ACTION.diff, { before: source, after: output });
        setDiff(compared.ok ? compared : null);
        if (!compared.ok) setMessage(errorText(compared));
      } else {
        setDiff(null);
        setMessage("agent 跑完了但没写出产物，看下面的运行日志。");
      }
    } finally {
      setRunning(false);
    }
  }, [source, output, agentId, payloadAnnotations, instruction]);

  return (
    <div className="flex h-screen flex-col bg-[#0f1115] text-gray-200">
      <header className="flex shrink-0 items-center gap-3 border-b border-white/10 px-4 py-2">
        <span className="text-[13px] font-semibold tracking-wide text-red-400">Redline</span>
        <button
          onClick={() => void pickFile()}
          className="rounded bg-white/10 px-3 py-1 text-[12px] hover:bg-white/20"
        >
          打开文件…
        </button>
        <span className="truncate text-[12px] text-gray-400" title={source ?? ""}>
          {source ?? "还没打开任何文件——Redline 打开任意格式都不需要装原软件"}
        </span>
      </header>

      {message && (
        <div className="shrink-0 border-b border-amber-500/30 bg-amber-500/10 px-4 py-1.5 text-[12px] text-amber-300">
          {message}
        </div>
      )}

      <main className="grid min-h-0 flex-1 grid-cols-[1fr_360px_1fr]">
        {/* 左：原文 + 标注 */}
        <section className="flex min-h-0 flex-col border-r border-white/10">
          <PaneTitle>原文（只读，永远不会被改）</PaneTitle>
          {source ? (
            <RedlinePanel
              host={tauriHost}
              path={source}
              fileName={baseName(source)}
              onAnnotationsChange={handleAnnotations}
              hideSendButton
            />
          ) : (
            <Empty>左边这栏是原文。圈选、画框、写批注都在这里。</Empty>
          )}
        </section>

        {/* 中：选 agent + 派发 */}
        <section className="flex min-h-0 flex-col overflow-auto border-r border-white/10">
          <PaneTitle>交给谁去改</PaneTitle>
          <div className="flex flex-col gap-3 p-3 text-[12px]">
            <div className="flex flex-col gap-1.5">
              {agents.map((agent) => (
                <label
                  key={agent.id}
                  className={`flex cursor-pointer items-start gap-2 rounded border px-2.5 py-2 ${
                    agentId === agent.id ? "border-red-400/60 bg-red-400/10" : "border-white/10 hover:bg-white/5"
                  } ${agent.installed ? "" : "opacity-50"}`}
                >
                  <input
                    type="radio"
                    className="mt-0.5"
                    checked={agentId === agent.id}
                    onChange={() => setAgentId(agent.id)}
                    disabled={!agent.installed}
                  />
                  <span className="min-w-0">
                    <span className="block font-medium">
                      {agent.label}
                      {!agent.installed && <span className="ml-1 text-gray-500">（没装）</span>}
                    </span>
                    {/* 授了什么权限必须写在明面上，不藏在代码里替用户做决定 */}
                    <span className="block text-[11px] leading-snug text-gray-400">{agent.permissionNote}</span>
                  </span>
                </label>
              ))}
            </div>

            <Field label="整体要求（可选）">
              <textarea
                value={instruction}
                onChange={(e) => setInstruction(e.target.value)}
                rows={3}
                placeholder="例如：语气正式一点，专有名词统一"
                className="w-full resize-y rounded border border-white/10 bg-black/30 px-2 py-1.5 outline-none focus:border-red-400/60"
              />
            </Field>

            <Field label={`标注（${annotations.length} 条）`}>
              {annotations.length === 0 ? (
                <p className="text-[11px] text-gray-500">
                  还没有标注。可以只写整体要求就派发，也可以先去左栏圈几处。
                </p>
              ) : (
                <ul className="flex flex-col gap-1">
                  {payloadAnnotations.map((a, i) => (
                    <li key={i} className="rounded bg-white/5 px-2 py-1 text-[11px]">
                      <span className="text-gray-500">{a.unitId ?? "未锚定"}</span> · {a.note}
                    </li>
                  ))}
                </ul>
              )}
            </Field>

            <Field label="产物写到">
              <input
                value={output}
                onChange={(e) => setOutput(e.target.value)}
                className="w-full rounded border border-white/10 bg-black/30 px-2 py-1.5 font-mono text-[11px] outline-none focus:border-red-400/60"
              />
              <p className="mt-1 text-[11px] text-gray-500">
                agent 写的是这个新文件，源文件全程不动。不满意就继续加标注再派一次。
              </p>
            </Field>

            <button
              onClick={() => void dispatchToAgent()}
              disabled={!source || !selectedAgent?.installed || running || !output}
              className="rounded bg-red-500 px-3 py-2 font-medium text-white enabled:hover:bg-red-400 disabled:cursor-not-allowed disabled:opacity-40"
            >
              {running ? `${selectedAgent?.label ?? "agent"} 正在改…` : "派发给 AI"}
            </button>

            {report && <RunLog report={report} />}
          </div>
        </section>

        {/* 右：产物 + 差异 */}
        <section className="flex min-h-0 flex-col">
          <PaneTitle>改后的新文件</PaneTitle>
          {diff && (
            <div className="shrink-0 border-b border-white/10 px-3 py-2 text-[11px]">
              <span className="text-gray-400">
                {diff.summary.changedUnits} 处改动 / 共 {diff.summary.totalUnits} 个单元
              </span>
              <ul className="mt-1.5 flex max-h-40 flex-col gap-1 overflow-auto">
                {diff.changes.map((change) => (
                  <li key={change.id} className="rounded bg-white/5 px-2 py-1">
                    <div className="text-gray-500">{change.label}</div>
                    <div className="text-red-300/80 line-through">{change.before.slice(0, 120)}</div>
                    <div className="text-emerald-300/90">{change.after.slice(0, 120)}</div>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {report?.outputExists ? (
            <RedlinePanel
              key={`${report.output}#${outputVersion}`}
              host={tauriHost}
              path={report.output}
              fileName={baseName(report.output)}
              hideSendButton
              readOnly
            />
          ) : (
            <Empty>派发之后，agent 产出的新文件会显示在这里，和左边逐处对照。</Empty>
          )}
        </section>
      </main>
    </div>
  );
}

function PaneTitle({ children }: { children: React.ReactNode }) {
  return (
    <h2 className="shrink-0 border-b border-white/10 px-3 py-1.5 text-[11px] font-medium uppercase tracking-wider text-gray-500">
      {children}
    </h2>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-1 text-[11px] font-medium text-gray-400">{label}</div>
      {children}
    </div>
  );
}

function Empty({ children }: { children: React.ReactNode }) {
  return <div className="flex flex-1 items-center justify-center p-6 text-center text-[12px] text-gray-600">{children}</div>;
}

function RunLog({ report }: { report: DispatchReport }) {
  return (
    <details className="rounded border border-white/10 bg-black/30 text-[11px]" open={!report.outputExists}>
      <summary className="cursor-pointer px-2 py-1.5">
        {report.outputExists ? "✅ 产物已生成" : "❌ 没生成产物"}
        <span className="ml-2 text-gray-500">
          退出码 {report.exitCode ?? "?"} · {report.durationMs ?? 0} ms
          {report.timedOut && " · 已超时被终止"}
        </span>
      </summary>
      <div className="max-h-56 overflow-auto px-2 pb-2 font-mono leading-relaxed text-gray-400">
        {/* 实际跑了什么命令要能被看见，包括那些放权参数 */}
        <div className="mb-1 text-gray-500">$ {report.command.join(" ")}</div>
        <pre className="whitespace-pre-wrap">{report.stderrTail || report.stdoutTail || "（没有输出）"}</pre>
      </div>
    </details>
  );
}
