/**
 * Redline 的主入口组件 —— 宿主只需要传 host + 文件路径，其余（格式识别、懒加载 viewer、
 * 标注持久化、unit 切换、发给 agent）全部自己搞定。这是"通用可插拔"落地的地方：
 * opencodex 的 FilesPanel 挂这个组件，以后任何宿主也是挂这一个组件，不用重新接线。
 */
import { Suspense, useCallback, useEffect, useMemo, useState } from "react";
import type { RedlineAnnotation, RedlineDocument, RedlineUnit } from "./document-model";
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
  /**
   * 标注变化时上抛给宿主。
   *
   * 三栏布局的宿主（左看原文、中选 agent 派发、右看产物）要在面板外面用这些标注。
   * 不给这个口子，宿主就只能自己再实现一遍标注状态 —— 那就成了第二份实现。
   */
  onAnnotationsChange?: (annotations: RedlineAnnotation[], doc: RedlineDocument | null) => void;
  /** 隐藏面板自带的「发给终端 Agent」按钮 —— 三栏布局里派发按钮在中间那栏。 */
  hideSendButton?: boolean;
  /** 只读预览模式：不画标注层。右栏看 agent 产物时用。 */
  readOnly?: boolean;
}

function mergeVisualUnits(doc: RedlineDocument, visualUnits: RedlineUnit[]): RedlineDocument {
  // 没有核心语义影子的宿主继续沿用 viewer units。
  if (!doc.shadow || doc.units.length === 0) return { ...doc, units: visualUnits };

  // 有 ShadowDoc 时，unit ID/文字/哈希以核心为准；viewer 只补视觉尺寸。
  const visualById = new Map(visualUnits.map((unit) => [unit.id, unit]));
  const sharedRenderSize = visualUnits.length === 1 ? visualUnits[0]?.renderSize : undefined;
  const units = doc.units.map((unit, index) => ({
    ...unit,
    index,
    renderSize: visualById.get(unit.id)?.renderSize ?? sharedRenderSize ?? unit.renderSize,
  }));
  return { ...doc, units };
}

export function RedlinePanel({
  host,
  path,
  fileName,
  onAnnotationsChange,
  hideSendButton,
  readOnly,
}: RedlinePanelProps) {
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
        // 语义 inspect 与读视觉字节并行，但不阻塞首屏：bytes 一到就先让 viewer 渲染，
        // ShadowDoc 随后异步合并，避免映射层拖慢“文件打不开，先用叠象”的第一体验。
        const inspectedPromise =
          host.inspectDocument?.(path).catch(() => null) ?? Promise.resolve(null);
        const b = await host.readFileBytes(path);
        if (cancelled) return;
        setBytes(b);
        setDoc({ docId: path, sourcePath: path, format, units: [] });

        const inspected = await inspectedPromise;
        if (cancelled || !inspected) return;
        setDoc((current) => mergeVisualUnits(inspected, current?.units ?? []));
        setActiveUnitId((current) =>
          inspected.units.some((unit) => unit.id === current)
            ? current
            : (inspected.units[0]?.id ?? current),
        );
      } catch (e) {
        if (!cancelled) setErr(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, path, format]);

  const handleUnitsResolved = useCallback((visualUnits: RedlineUnit[]) => {
    setDoc((d) => {
      if (!d) return d;
      return mergeVisualUnits(d, visualUnits);
    });
    setActiveUnitId((cur) => cur || visualUnits[0]?.id || "");
  }, []);

  const Viewer = useMemo(() => loadViewer(format), [format]);

  useEffect(() => {
    onAnnotationsChange?.(annotations, doc);
  }, [annotations, doc, onAnnotationsChange]);

  const renderOverlay = useCallback(
    (unitId: string) =>
      readOnly ? null : (
        <AnnotationLayer
          unitId={unitId}
          annotations={annotations.filter((a) => a.unitId === unitId)}
          onAdd={addAnnotation}
          onRemove={removeAnnotation}
        />
      ),
    [annotations, addAnnotation, removeAnnotation, readOnly],
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
      {text && host.sendToAgent && !hideSendButton && (
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
