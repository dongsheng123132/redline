/**
 * Redline 的主入口组件 —— 宿主只需要传 host + 文件路径，其余（格式识别、懒加载 viewer、
 * 标注持久化、unit 切换、发给 agent）全部自己搞定。这是"通用可插拔"落地的地方：
 * opencodex 的 FilesPanel 挂这个组件，以后任何宿主也是挂这一个组件，不用重新接线。
 */
import { Suspense, useCallback, useEffect, useMemo, useState } from "react";
import type { RedlineDocument, RedlineUnit } from "./document-model";
import type { RedlineHost } from "./host-adapter";
import { detectFormat, loadViewer } from "./viewers/registry";
import { AnnotationLayer } from "./annotation/AnnotationLayer";
import { useAnnotations } from "./annotation/useAnnotations";
import { UnitSwitcher } from "./UnitSwitcher";

export interface RedlinePanelProps {
  host: RedlineHost;
  /** 文件绝对路径。 */
  path: string;
  /** 展示/发给 agent 时用的文件名（一般就是 path 的 basename，交由调用方决定怎么截）。 */
  fileName: string;
}

export function RedlinePanel({ host, path, fileName }: RedlinePanelProps) {
  const format = useMemo(() => detectFormat(fileName), [fileName]);
  const [bytes, setBytes] = useState<ArrayBuffer | null>(null);
  const [doc, setDoc] = useState<RedlineDocument | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [activeUnitId, setActiveUnitId] = useState<string>("");

  const { annotations, addAnnotation, removeAnnotation, exportText } = useAnnotations(host, doc);

  useEffect(() => {
    let cancelled = false;
    setBytes(null);
    setDoc(null);
    setErr(null);
    setActiveUnitId("");
    (async () => {
      try {
        const b = await host.readFileBytes(path);
        if (cancelled) return;
        setBytes(b);
        setDoc({ docId: path, sourcePath: path, format, units: [] });
      } catch (e) {
        if (!cancelled) setErr(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, path, format]);

  const handleUnitsResolved = useCallback((units: RedlineUnit[]) => {
    setDoc((d) => (d ? { ...d, units } : d));
    setActiveUnitId((cur) => cur || units[0]?.id || "");
  }, []);

  const Viewer = useMemo(() => loadViewer(format), [format]);

  const renderOverlay = useCallback(
    (unitId: string) => (
      <AnnotationLayer
        unitId={unitId}
        annotations={annotations.filter((a) => a.unitId === unitId)}
        onAdd={addAnnotation}
        onRemove={removeAnnotation}
      />
    ),
    [annotations, addAnnotation, removeAnnotation],
  );

  const openExternal = host.openExternal ? () => host.openExternal!(path) : undefined;
  const text = exportText();

  if (err) {
    return <div className="p-4 text-red-400 text-[12px]">打开失败：{err}</div>;
  }
  if (!bytes || !doc) {
    return <div className="p-4 text-gray-400 text-[12px]">加载中…</div>;
  }

  return (
    <div className="flex flex-col h-full min-h-0">
      {doc.units.length > 1 && (
        <UnitSwitcher units={doc.units} activeUnitId={activeUnitId} onChange={setActiveUnitId} />
      )}
      <Suspense fallback={<div className="p-4 text-gray-400 text-[12px]">渲染中…</div>}>
        <Viewer
          doc={doc}
          bytes={bytes}
          srcUrl={host.getSrcUrl?.(path)}
          activeUnitId={activeUnitId}
          onUnitsResolved={handleUnitsResolved}
          renderOverlay={renderOverlay}
          openExternal={openExternal}
        />
      </Suspense>
      {text && host.sendToAgent && (
        <div className="shrink-0 border-t border-white/[0.06] p-2">
          <button
            onClick={() => void host.sendToAgent!(text)}
            className="w-full text-[12px] px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white"
          >
            发给终端 Agent（{annotations.length} 条标注）
          </button>
        </div>
      )}
    </div>
  );
}
