/**
 * 宿主适配器 —— redline-core 对外唯一依赖的接口。
 *
 * 内核不假设自己活在 Tauri / Electron / Node 里，也不直接碰任何平台 API（文件系统、
 * 持久化、进程通信……）。这三件事宿主各自实现一遍，内核只认这个接口，
 * 换宿主时（opencodex → MCP Server → 以后随便什么）viewer/annotation 代码一行都不用改。
 */
import type { RedlineDocument } from "./document-model";

/** 标注持久化。宿主决定存哪——opencodex 走现成的 kv.rs，MCP Server 可以是本地 json/sqlite。 */
export interface RedlineAnnotationStore {
  get(key: string): Promise<string | null>;
  /** value 为 null 表示删除该 key。 */
  set(key: string, value: string | null): Promise<void>;
}

export interface RedlineHost {
  /**
   * 拿文件原始字节。Redline 从不自己决定"怎么访问文件系统"——
   * opencodex 里是 `fetch(convertFileSrc(path))`，MCP Server 里可以直接 `fs.readFile`。
   */
  readFileBytes(path: string): Promise<ArrayBuffer>;
  /**
   * 可选：调用宿主的 `document.inspect`，拿到与原件哈希绑定的语义影子。
   * viewer 仍负责视觉渲染，但 unit ID、文字和内容哈希以这个结果为准。
   */
  inspectDocument?: (path: string) => Promise<RedlineDocument>;
  /** 标注持久化。 */
  annotationStore: RedlineAnnotationStore;
  /**
   * 可选：把一段文字发给某个 agent。opencodex 里是写入当前聚焦的终端会话（不自动回车）；
   * 没实现这个方法的宿主，标注只能靠用户手动复制走。
   */
  sendToAgent?: (text: string) => Promise<void>;
  /** 可选：用系统默认程序打开文件——渲染失败/不支持的格式兜底用。 */
  openExternal?: (path: string) => Promise<void>;
  /**
   * 可选：给文件一个宿主原生可访问的 URL（比如 Tauri 的 convertFileSrc），
   * 图片/html 这类可以直接当 src 用，省一次「读 bytes 再拼 blob URL」的内存拷贝。
   * 没提供就退化成从 bytes 生成 blob URL，效果一样，只是稍慢。
   */
  getSrcUrl?: (path: string) => string;
}

/** 标注存储的 key 约定：按文件路径的稳定 hash 存，避免路径里的中文/特殊字符污染 key。 */
export function annotationStoreKey(sourcePath: string, sourceSha256?: string): string {
  const identity = sourceSha256 ? `${sourcePath}\0${sourceSha256}` : sourcePath;
  let h = 0;
  for (let i = 0; i < identity.length; i++) {
    h = (Math.imul(31, h) + identity.charCodeAt(i)) | 0;
  }
  return `redline:annot:${(h >>> 0).toString(36)}`;
}
