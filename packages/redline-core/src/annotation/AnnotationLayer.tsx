/**
 * 标注覆盖层 —— SVG 叠加，框/箭头/画笔/批注四种工具，产出结构化数据。
 *
 * 坐标系用 viewBox="0 0 100 100" + preserveAspectRatio="none"，所有点位按 0~100 的
 * 分数坐标存（对应 document-model 里 bbox/points 的 0~1 约定，这里 ×100 只是 SVG 单位习惯）。
 * 好处：容器缩放/窗口改变大小/换成全分辨率截图，标注位置永远对得上，不用做换算。
 *
 * 浏览模式（view）下整个 SVG pointer-events:none，不挡住底下内容的滚动/选择/3D 拖拽；
 * 已有的标注形状单独设 pointer-events:auto，浏览模式下仍然可以点开看/删。
 */
import { useRef, useState } from "react";
import type { RedlineAnnotation, RedlineAnnotationKind } from "../document-model";

type Mode = "view" | RedlineAnnotationKind;
type Point = { x: number; y: number };

interface Draft {
  kind: RedlineAnnotationKind;
  start: Point;
  points: Point[];
}

interface PendingNote {
  kind: RedlineAnnotationKind;
  bbox?: { x: number; y: number; width: number; height: number };
  points?: Point[];
  anchor: Point;
}

export interface AnnotationLayerProps {
  unitId: string;
  annotations: RedlineAnnotation[];
  onAdd: (a: RedlineAnnotation) => void;
  onRemove?: (id: string) => void;
}

const TOOLS: { mode: Mode; label: string }[] = [
  { mode: "view", label: "浏览" },
  { mode: "rect", label: "框" },
  { mode: "arrow", label: "箭头" },
  { mode: "freehand", label: "画笔" },
  { mode: "note", label: "批注" },
];

let seq = 0;
function nextId(): string {
  seq += 1;
  return `a${Date.now().toString(36)}${seq.toString(36)}`;
}

const MIN_SIZE = 0.005; // 太小的框/画笔视为误触，不生成标注

export function AnnotationLayer({ unitId, annotations, onAdd, onRemove }: AnnotationLayerProps) {
  const [mode, setMode] = useState<Mode>("view");
  const svgRef = useRef<SVGSVGElement>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [pendingNote, setPendingNote] = useState<PendingNote | null>(null);
  const [noteText, setNoteText] = useState("");
  const [viewing, setViewing] = useState<RedlineAnnotation | null>(null);

  const toFrac = (e: { clientX: number; clientY: number }): Point => {
    const rect = svgRef.current!.getBoundingClientRect();
    return {
      x: Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width)),
      y: Math.min(1, Math.max(0, (e.clientY - rect.top) / rect.height)),
    };
  };

  const onPointerDown = (e: React.PointerEvent<SVGSVGElement>) => {
    if (mode === "view" || pendingNote) return;
    const p = toFrac(e);
    if (mode === "note") {
      setPendingNote({ kind: "note", bbox: { x: p.x, y: p.y, width: 0, height: 0 }, anchor: p });
      return;
    }
    svgRef.current?.setPointerCapture(e.pointerId);
    setDraft({ kind: mode, start: p, points: [p] });
  };

  const onPointerMoveHandler = (e: React.PointerEvent<SVGSVGElement>) => {
    if (!draft) return;
    const p = toFrac(e);
    setDraft((d) => (d ? { ...d, points: d.kind === "freehand" ? [...d.points, p] : [d.start, p] } : d));
  };

  const onPointerUpHandler = () => {
    if (!draft) return;
    const last = draft.points[draft.points.length - 1] ?? draft.start;
    if (draft.kind === "arrow") {
      setPendingNote({ kind: "arrow", points: [draft.start, last], anchor: last });
    } else if (draft.kind === "rect") {
      const bbox = {
        x: Math.min(draft.start.x, last.x),
        y: Math.min(draft.start.y, last.y),
        width: Math.abs(last.x - draft.start.x),
        height: Math.abs(last.y - draft.start.y),
      };
      if (bbox.width < MIN_SIZE || bbox.height < MIN_SIZE) {
        setDraft(null);
        return;
      }
      setPendingNote({ kind: "rect", bbox, anchor: { x: bbox.x, y: bbox.y } });
    } else if (draft.kind === "freehand") {
      if (draft.points.length < 2) {
        setDraft(null);
        return;
      }
      setPendingNote({ kind: "freehand", points: draft.points, anchor: draft.points[0] });
    }
    setDraft(null);
  };

  const commitNote = () => {
    if (!pendingNote) return;
    onAdd({
      id: nextId(),
      unitId,
      kind: pendingNote.kind,
      bbox: pendingNote.bbox,
      points: pendingNote.points,
      note: noteText.trim() || "（无说明）",
      createdAt: Date.now(),
    });
    setPendingNote(null);
    setNoteText("");
  };
  const cancelNote = () => {
    setPendingNote(null);
    setNoteText("");
  };

  return (
    <div className="absolute inset-0">
      <svg
        ref={svgRef}
        viewBox="0 0 100 100"
        preserveAspectRatio="none"
        className="absolute inset-0 w-full h-full"
        style={{ pointerEvents: mode === "view" ? "none" : "auto", touchAction: "none" }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMoveHandler}
        onPointerUp={onPointerUpHandler}
      >
        <defs>
          <marker id="redline-arrowhead" markerWidth="6" markerHeight="6" refX="5" refY="3" orient="auto">
            <path d="M0,0 L6,3 L0,6 z" fill="#ef4444" />
          </marker>
        </defs>
        {annotations.map((a) => (
          <AnnotationShape key={a.id} a={a} onClick={() => mode === "view" && setViewing(a)} />
        ))}
        {draft && <DraftShape draft={draft} />}
      </svg>

      {pendingNote && (
        <NotePopup
          anchor={pendingNote.anchor}
          text={noteText}
          onChange={setNoteText}
          onCommit={commitNote}
          onCancel={cancelNote}
        />
      )}

      {viewing && (
        <div
          className="absolute z-10 bg-neutral-900 text-white border border-white/20 rounded p-2 shadow-xl max-w-[220px] text-[12px]"
          style={{ left: `${anchorOf(viewing).x * 100}%`, top: `${anchorOf(viewing).y * 100}%` }}
        >
          <div className="whitespace-pre-wrap mb-1.5">{viewing.note}</div>
          <div className="flex gap-1.5 justify-end">
            <button
              onClick={() => setViewing(null)}
              className="text-[11px] px-2 py-0.5 rounded hover:bg-white/10 text-white/60"
            >
              关闭
            </button>
            {onRemove && (
              <button
                onClick={() => {
                  onRemove(viewing.id);
                  setViewing(null);
                }}
                className="text-[11px] px-2 py-0.5 rounded bg-red-600 hover:bg-red-500"
              >
                删除
              </button>
            )}
          </div>
        </div>
      )}

      <div className="absolute top-1.5 right-1.5 flex gap-0.5 bg-black/60 rounded p-0.5 backdrop-blur-sm z-10">
        {TOOLS.map((t) => (
          <button
            key={t.mode}
            onClick={() => setMode(t.mode)}
            className={
              "text-[11px] px-2 py-1 rounded " +
              (mode === t.mode ? "bg-blue-600 text-white" : "text-white/70 hover:bg-white/10")
            }
          >
            {t.label}
          </button>
        ))}
      </div>
    </div>
  );
}

function anchorOf(a: RedlineAnnotation): Point {
  if (a.bbox) return { x: a.bbox.x, y: a.bbox.y };
  if (a.points?.length) return a.points[0];
  return { x: 0, y: 0 };
}

function AnnotationShape({ a, onClick }: { a: RedlineAnnotation; onClick: () => void }) {
  const common = { style: { pointerEvents: "auto" as const, cursor: "pointer" }, onClick };
  if (a.kind === "rect" && a.bbox) {
    return (
      <rect
        x={a.bbox.x * 100}
        y={a.bbox.y * 100}
        width={a.bbox.width * 100}
        height={a.bbox.height * 100}
        fill="rgba(239,68,68,0.12)"
        stroke="#ef4444"
        strokeWidth={0.4}
        {...common}
      />
    );
  }
  if (a.kind === "arrow" && a.points?.length === 2) {
    const [p1, p2] = a.points;
    return (
      <line
        x1={p1.x * 100}
        y1={p1.y * 100}
        x2={p2.x * 100}
        y2={p2.y * 100}
        stroke="#ef4444"
        strokeWidth={0.5}
        markerEnd="url(#redline-arrowhead)"
        {...common}
      />
    );
  }
  if (a.kind === "freehand" && a.points?.length) {
    return (
      <polyline
        points={a.points.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")}
        fill="none"
        stroke="#ef4444"
        strokeWidth={0.5}
        strokeLinecap="round"
        strokeLinejoin="round"
        {...common}
      />
    );
  }
  if (a.kind === "note" && a.bbox) {
    return (
      <circle cx={a.bbox.x * 100} cy={a.bbox.y * 100} r={1.4} fill="#ef4444" stroke="#fff" strokeWidth={0.3} {...common} />
    );
  }
  return null;
}

function DraftShape({ draft }: { draft: Draft }) {
  const last = draft.points[draft.points.length - 1] ?? draft.start;
  if (draft.kind === "rect") {
    const x = Math.min(draft.start.x, last.x) * 100;
    const y = Math.min(draft.start.y, last.y) * 100;
    const w = Math.abs(last.x - draft.start.x) * 100;
    const h = Math.abs(last.y - draft.start.y) * 100;
    return <rect x={x} y={y} width={w} height={h} fill="rgba(239,68,68,0.12)" stroke="#ef4444" strokeWidth={0.4} strokeDasharray="1,1" />;
  }
  if (draft.kind === "arrow") {
    return (
      <line
        x1={draft.start.x * 100}
        y1={draft.start.y * 100}
        x2={last.x * 100}
        y2={last.y * 100}
        stroke="#ef4444"
        strokeWidth={0.5}
        strokeDasharray="1,1"
      />
    );
  }
  return (
    <polyline
      points={draft.points.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")}
      fill="none"
      stroke="#ef4444"
      strokeWidth={0.5}
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  );
}

function NotePopup({
  anchor,
  text,
  onChange,
  onCommit,
  onCancel,
}: {
  anchor: Point;
  text: string;
  onChange: (v: string) => void;
  onCommit: () => void;
  onCancel: () => void;
}) {
  return (
    <div
      className="absolute z-10 bg-neutral-900 border border-white/20 rounded p-2 shadow-xl flex flex-col gap-1.5"
      style={{ left: `${anchor.x * 100}%`, top: `${anchor.y * 100}%` }}
    >
      <textarea
        autoFocus
        value={text}
        onChange={(e) => onChange(e.target.value)}
        placeholder="这里有什么问题？"
        className="w-56 h-16 text-[12px] bg-black/40 text-white rounded p-1.5 outline-none resize-none"
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) onCommit();
          if (e.key === "Escape") onCancel();
        }}
      />
      <div className="flex gap-1.5 justify-end">
        <button onClick={onCancel} className="text-[11px] px-2 py-0.5 rounded hover:bg-white/10 text-white/60">
          取消
        </button>
        <button onClick={onCommit} className="text-[11px] px-2 py-0.5 rounded bg-blue-600 hover:bg-blue-500 text-white">
          确定 ⌘⏎
        </button>
      </div>
    </div>
  );
}
