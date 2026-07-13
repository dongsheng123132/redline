import { useEffect, useRef, useState } from "react";
import type { ViewerProps } from "./types";
import { singleUnit, useObjectUrl } from "./util";

/** HTML 预览 —— 从现有 opencodex FilesPanel 的 html 分支迁移（sandbox iframe），加了标注覆盖层。 */
export default function HtmlViewer({ bytes, srcUrl, activeUnitId, onUnitsResolved, renderOverlay }: ViewerProps) {
  const url = useObjectUrl(bytes, srcUrl, "text/html");
  const wrapRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const report = () => {
      const s = { width: el.clientWidth, height: el.clientHeight };
      setSize(s);
      onUnitsResolved([singleUnit("网页", s)]);
    };
    report();
    const ro = new ResizeObserver(report);
    ro.observe(el);
    return () => ro.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div ref={wrapRef} className="relative flex-1 w-full min-h-0">
      <iframe
        src={url}
        title="html-preview"
        className="w-full h-full border-0 bg-white"
        sandbox="allow-scripts allow-same-origin allow-forms"
      />
      {size && <div className="absolute inset-0">{renderOverlay(activeUnitId, size)}</div>}
    </div>
  );
}
