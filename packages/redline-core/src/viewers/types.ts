import type { ReactNode } from "react";
import type { RedlineDocument, RedlineUnit } from "../document-model";

/**
 * 每个 viewer 的统一 props。viewer 只管"把 bytes 渲染成看得见的东西"，
 * 不关心标注层怎么画——渲染出当前 unit 的原生像素尺寸后调用 renderOverlay 拿到
 * 标注覆盖层节点，塞进自己布局里的对应位置（一般是包一层 position:relative 的容器）。
 */
export interface ViewerProps {
  doc: RedlineDocument;
  /** 文件原始字节，大部分格式解析要用。 */
  bytes: ArrayBuffer;
  /** 宿主提供的原生可访问 URL（比如 Tauri 的 convertFileSrc），能省一次内存拷贝；
   * 没有则 viewer 自己从 bytes 生成 blob URL 兜底。 */
  srcUrl?: string;
  activeUnitId: string;
  /** 多 unit 格式（PDF 页数、PSD 图层、ZIP 条目……）只有解析后才知道具体有哪些 unit，
   * 解析完必须调用一次，FilesPanel 才能画出 unit 切换 UI（页码/图层列表等）。 */
  onUnitsResolved: (units: RedlineUnit[]) => void;
  /** 拿到当前 unit 的标注覆盖层节点，按 renderSize 定位。 */
  renderOverlay: (unitId: string, renderSize: { width: number; height: number }) => ReactNode;
  /** 用系统默认程序打开这份文档——渲染失败/不支持格式时兜底；宿主没实现就不展示按钮。 */
  openExternal?: () => Promise<void>;
}
