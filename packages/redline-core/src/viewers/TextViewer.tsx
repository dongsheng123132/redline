import { useEffect, useMemo, useRef, useState } from "react";
import type { ViewerProps } from "./types";
import { singleUnit } from "./util";

const MAX_BYTES = 256 * 1024;

/**
 * 纯文本预览。
 *
 * 分块规则只在 Rust 核心实现；有 ShadowDoc 时 viewer 直接展示 active unit 的正文，
 * 没有核心宿主时才退化为本地全文预览。
 */
export default function TextViewer({
  doc,
  bytes,
  activeUnitId,
  onUnitsResolved,
  renderOverlay,
}: ViewerProps) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);

  const text = useMemo(() => {
    const view = new Uint8Array(bytes);
    const slice = view.subarray(0, Math.min(view.length, MAX_BYTES));
    if (slice.includes(0)) return null; // 含 NUL 视为二进制，不预览
    let s = new TextDecoder("utf-8", { fatal: false }).decode(slice);
    if (view.length > MAX_BYTES) s += "\n\n…（文件过大，仅显示前 256KB）";
    return s;
  }, [bytes]);

  useEffect(() => {
    onUnitsResolved([singleUnit("文本")]);
  }, [onUnitsResolved]);

  const activeUnit = doc.shadow
    ? (doc.units.find((unit) => unit.id === activeUnitId) ?? doc.units[0])
    : undefined;
  const visibleText = activeUnit?.text ?? text;

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const report = () => setSize({ width: el.clientWidth, height: el.clientHeight });
    report();
    const ro = new ResizeObserver(report);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  return (
    <div ref={wrapRef} className="relative flex-1 overflow-auto min-h-0">
      <pre className="p-3 text-[12px] leading-relaxed font-mono whitespace-pre-wrap">
        {visibleText ?? "[无法预览: 二进制文件]"}
      </pre>
      {size && <div className="absolute inset-0">{renderOverlay(activeUnitId, size)}</div>}
    </div>
  );
}
