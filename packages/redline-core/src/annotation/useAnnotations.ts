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
  const sourcePath = doc?.sourcePath;
  const sourceSha256 = doc?.sourceSha256;

  useEffect(() => {
    let cancelled = false;
    setLoaded(false);
    setAnnotations([]);
    if (!sourcePath) return;
    (async () => {
      // 有源哈希时绝不回退读取旧的「只按路径」标注；同一路径换了文件内容，
      // 旧 bbox/unitId 不能静默套到新原件上。
      const raw = await host.annotationStore.get(annotationStoreKey(sourcePath, sourceSha256));
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
  }, [host, sourcePath, sourceSha256]);

  const persist = useCallback(
    (next: RedlineAnnotation[]) => {
      if (!sourcePath) return;
      void host.annotationStore.set(annotationStoreKey(sourcePath, sourceSha256), JSON.stringify(next));
    },
    [host, sourcePath, sourceSha256],
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
