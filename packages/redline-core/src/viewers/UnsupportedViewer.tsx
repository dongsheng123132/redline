import { useEffect } from "react";
import type { ViewerProps } from "./types";
import { singleUnit } from "./util";

/** 兜底 viewer —— 不支持的格式给一个"用默认程序打开"按钮，不是一个报错死胡同。 */
export default function UnsupportedViewer({ doc, onUnitsResolved, openExternal }: ViewerProps) {
  useEffect(() => {
    onUnitsResolved([singleUnit("不支持预览")]);
  }, [onUnitsResolved]);

  return (
    <div className="h-full flex flex-col items-center justify-center gap-3 text-[12px] text-gray-400 p-6 text-center">
      <div>Redline 暂不支持预览这个格式（{doc.sourcePath.split(".").pop()}）</div>
      {openExternal && (
        <button
          onClick={() => void openExternal()}
          className="px-3 py-1.5 rounded border border-current hover:bg-white/10"
        >
          用默认程序打开
        </button>
      )}
    </div>
  );
}
