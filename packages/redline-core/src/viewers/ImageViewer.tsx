import { useRef, useState } from "react";
import type { ViewerProps } from "./types";
import { singleUnit, useObjectUrl } from "./util";

/** 图片预览 —— 从现有 opencodex FilesPanel 的 image 分支迁移，加了标注覆盖层。 */
export default function ImageViewer({ bytes, srcUrl, activeUnitId, onUnitsResolved, renderOverlay }: ViewerProps) {
  const url = useObjectUrl(bytes, srcUrl, "image/*");
  const imgRef = useRef<HTMLImageElement>(null);
  const [ready, setReady] = useState(false);

  const handleLoad = () => {
    const img = imgRef.current;
    if (!img) return;
    setReady(true);
    onUnitsResolved([singleUnit("图片", { width: img.naturalWidth, height: img.naturalHeight })]);
  };

  return (
    <div className="flex-1 overflow-auto p-4 flex items-center justify-center bg-black/20">
      <div className="relative inline-block max-w-full max-h-full">
        <img
          ref={imgRef}
          src={url}
          onLoad={handleLoad}
          alt=""
          className="block max-w-full max-h-full object-contain"
        />
        {ready && (
          <div className="absolute inset-0">
            {renderOverlay(activeUnitId, {
              width: imgRef.current?.naturalWidth ?? 0,
              height: imgRef.current?.naturalHeight ?? 0,
            })}
          </div>
        )}
      </div>
    </div>
  );
}
