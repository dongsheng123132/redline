import { useEffect, useState } from "react";
import type { ViewerProps } from "./types";
import type { RedlineUnit } from "../document-model";

/** PDF 逐页渲染。pdf.js 懒加载，worker 走 Vite `?url` 资产导入，不进主 bundle。 */
export default function PdfViewer({ bytes, activeUnitId, onUnitsResolved, renderOverlay }: ViewerProps) {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [pdf, setPdf] = useState<any>(null);
  const [err, setErr] = useState<string | null>(null);
  const [canvasEl, setCanvasEl] = useState<HTMLCanvasElement | null>(null);
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);

  useEffect(() => {
    let cancelled = false;
    setPdf(null);
    setSize(null);
    (async () => {
      try {
        const pdfjsLib = await import("pdfjs-dist");
        // 用标准 ESM `new URL(..., import.meta.url)` 取 worker 地址，不用 Vite 专属的 `?url`
        // 后缀语法——Vite/Rollup/大多数现代打包器都认这个写法，redline-core 不用为了这一行
        // 绑死某个打包器，纯 tsc 独立 typecheck 也能过。
        const workerUrl = new URL("pdfjs-dist/build/pdf.worker.mjs", import.meta.url).href;
        pdfjsLib.GlobalWorkerOptions.workerSrc = workerUrl;
        const doc = await pdfjsLib.getDocument({ data: bytes.slice(0) }).promise;
        if (cancelled) return;
        const units: RedlineUnit[] = Array.from({ length: doc.numPages }, (_, i) => ({
          id: String(i + 1),
          index: i,
          label: `第 ${i + 1} 页`,
        }));
        onUnitsResolved(units);
        setPdf(doc);
      } catch (e) {
        if (!cancelled) setErr(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [bytes, onUnitsResolved]);

  useEffect(() => {
    let cancelled = false;
    if (!pdf || !canvasEl || !activeUnitId) return;
    (async () => {
      const page = await pdf.getPage(Number(activeUnitId));
      const viewport = page.getViewport({ scale: 1.5 });
      canvasEl.width = viewport.width;
      canvasEl.height = viewport.height;
      const ctx = canvasEl.getContext("2d");
      if (!ctx) return;
      await page.render({ canvasContext: ctx, viewport }).promise;
      if (!cancelled) setSize({ width: viewport.width, height: viewport.height });
    })();
    return () => {
      cancelled = true;
    };
  }, [pdf, canvasEl, activeUnitId]);

  if (err) {
    return <div className="p-4 text-red-400 text-[12px]">PDF 解析失败：{err}</div>;
  }

  return (
    <div className="flex-1 overflow-auto p-4 flex items-start justify-center bg-black/20">
      <div className="relative inline-block">
        <canvas ref={setCanvasEl} className="block shadow-lg" />
        {size && activeUnitId && (
          <div className="absolute inset-0">{renderOverlay(activeUnitId, size)}</div>
        )}
      </div>
    </div>
  );
}
