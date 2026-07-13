import { useEffect, useRef, useState } from "react";
import type { ViewerProps } from "./types";
import { singleUnit } from "./util";

/**
 * Word 只读预览 —— mammoth.js 把 docx 转成语义化 html（mammoth 自己生成标签，
 * 不透传文件里的原始标记），用 dangerouslySetInnerHTML 直接渲染风险很低，
 * 不像 sheet_to_html 那样可能把单元格原值透传进 DOM（那个用 React 转义，见 SheetViewer）。
 */
export default function DocxViewer({ bytes, activeUnitId, onUnitsResolved, renderOverlay, openExternal }: ViewerProps) {
  const [html, setHtml] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const mammoth = await import("mammoth/mammoth.browser");
        const result = await mammoth.convertToHtml({ arrayBuffer: bytes });
        if (cancelled) return;
        setHtml(result.value);
        const text = result.value.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
        onUnitsResolved([singleUnit("Word 文档", undefined, text)]);
      } catch (e) {
        if (!cancelled) setErr(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [bytes, onUnitsResolved]);

  useEffect(() => {
    const el = wrapRef.current;
    if (!el || !html) return;
    const report = () => setSize({ width: el.clientWidth, height: el.clientHeight });
    report();
    const ro = new ResizeObserver(report);
    ro.observe(el);
    return () => ro.disconnect();
  }, [html]);

  if (err) {
    return (
      <div className="p-4 flex flex-col gap-2 text-[12px]">
        <div className="text-red-400">Word 解析失败：{err}</div>
        {openExternal && (
          <button
            onClick={() => void openExternal()}
            className="self-start px-3 py-1.5 rounded border border-current hover:bg-white/10"
          >
            用默认程序打开
          </button>
        )}
      </div>
    );
  }

  return (
    <div className="relative flex-1 overflow-auto min-h-0 bg-white text-black">
      <div
        ref={wrapRef}
        className="max-w-[820px] mx-auto p-8 text-[13px] leading-relaxed [&_h1]:text-xl [&_h1]:font-bold [&_h2]:text-lg [&_h2]:font-bold [&_table]:border-collapse [&_td]:border [&_td]:border-gray-300 [&_td]:px-2 [&_td]:py-1"
        dangerouslySetInnerHTML={{ __html: html ?? "" }}
      />
      {size && html && <div className="absolute inset-0">{renderOverlay(activeUnitId, size)}</div>}
    </div>
  );
}
