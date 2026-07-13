/**
 * 标注状态 + 持久化 —— 包一层宿主的 annotationStore（opencodex 里是 kv.rs），
 * 不关心具体存哪，宿主换了这个 hook 不用改。
 */
import { useCallback, useEffect, useState } from "react";
import type { RedlineHost } from "../host-adapter";
import { annotationStoreKey } from "../host-adapter";
import type { RedlineAnnotation, RedlineAnnotationExport, RedlineDocument } from "../document-model";
import { exportAnnotations, formatAnnotationsAsText } from "../document-model";

export function useAnnotations(host: RedlineHost, doc: RedlineDocument | null) {
  const [annotations, setAnnotations] = useState<RedlineAnnotation[]>([]);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoaded(false);
    setAnnotations([]);
    if (!doc) return;
    (async () => {
      const raw = await host.annotationStore.get(annotationStoreKey(doc.sourcePath));
      if (cancelled) return;
      try {
        setAnnotations(raw ? (JSON.parse(raw) as RedlineAnnotation[]) : []);
      } catch {
        setAnnotations([]);
      }
      setLoaded(true);
    })();
    return () => {
      cancelled = true;
    };
  }, [host, doc]);

  const persist = useCallback(
    (next: RedlineAnnotation[]) => {
      if (!doc) return;
      void host.annotationStore.set(annotationStoreKey(doc.sourcePath), JSON.stringify(next));
    },
    [host, doc],
  );

  const addAnnotation = useCallback(
    (a: RedlineAnnotation) => {
      setAnnotations((prev) => {
        const next = [...prev, a];
        persist(next);
        return next;
      });
    },
    [persist],
  );

  const removeAnnotation = useCallback(
    (id: string) => {
      setAnnotations((prev) => {
        const next = prev.filter((a) => a.id !== id);
        persist(next);
        return next;
      });
    },
    [persist],
  );

  /** 把当前全部标注格式化成一段文字，供"发给终端"按钮直接写入 agent 的输入。 */
  const exportText = useCallback((): string | null => {
    if (!doc || annotations.length === 0) return null;
    const exported: RedlineAnnotationExport = exportAnnotations(doc, annotations);
    return formatAnnotationsAsText(exported, doc);
  }, [doc, annotations]);

  return { annotations, loaded, addAnnotation, removeAnnotation, exportText };
}
