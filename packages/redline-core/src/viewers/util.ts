import { useEffect, useState } from "react";
import type { RedlineUnit } from "../document-model";

/** 单 unit 格式（图片/html/text/docx……）共用的 unit 构造。 */
export function singleUnit(label: string, renderSize?: { width: number; height: number }, text?: string): RedlineUnit {
  return { id: "document:1", kind: "document", index: 0, label, renderSize, text };
}

/** 跟 Rust inspect 的 `page:1` / `sheet:1` / `slide:1` 规则保持一致。 */
export function numberedUnitId(kind: string, zeroBasedIndex: number): string {
  return `${kind}:${zeroBasedIndex + 1}`;
}

/** 把一基 unit id 还原成零基下标；格式不对时返回 -1，不猜。 */
export function numberedUnitIndex(id: string, kind: string): number {
  const prefix = `${kind}:`;
  if (!id.startsWith(prefix)) return -1;
  const oneBased = Number(id.slice(prefix.length));
  return Number.isInteger(oneBased) && oneBased > 0 ? oneBased - 1 : -1;
}

/** 优先用宿主给的 srcUrl（比如 Tauri convertFileSrc，省一次拷贝），没有就从 bytes 生成 blob URL，卸载时自动释放。 */
export function useObjectUrl(bytes: ArrayBuffer, srcUrl: string | undefined, mime: string): string {
  const [url, setUrl] = useState(srcUrl ?? "");
  useEffect(() => {
    if (srcUrl) {
      setUrl(srcUrl);
      return;
    }
    const blobUrl = URL.createObjectURL(new Blob([bytes], { type: mime }));
    setUrl(blobUrl);
    return () => URL.revokeObjectURL(blobUrl);
  }, [bytes, srcUrl, mime]);
  return url;
}
