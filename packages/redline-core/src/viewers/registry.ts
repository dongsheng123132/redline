/**
 * 格式注册表（渲染层）—— **这份表不是真相源**。
 *
 * 「支持哪些格式、扩展名怎么映射、哪个格式用哪个 viewer」的唯一权威在
 * `crates/redline-core/src/format.rs`。这里只保留渲染层跑起来必须的东西
 * （懒加载哪个组件），并且必须跟 Rust 那份一致 —— 靠 `scripts/check-format-parity.mjs`
 * 守着，漂了就挂。
 *
 * 为什么不干脆运行时从核心拉：viewer 走 React.lazy 静态 import，打包器要在构建期
 * 看得见这些路径才能分包。所以表结构留在这里，一致性交给检查脚本。
 *
 * 所有 viewer 都走 React.lazy + 动态 import，用户真正打开某个格式时才拉对应库
 * （pdf.js/mammoth/xlsx/three.js……），不进主 bundle，守住宿主的体积红线。
 */
import { lazy, type ComponentType } from "react";
import type { RedlineFormat } from "../document-model";
import type { ViewerProps } from "./types";

const EXT_TO_FORMAT: Record<string, RedlineFormat> = {
  png: "image", jpg: "image", jpeg: "image", gif: "image", webp: "image",
  svg: "image", bmp: "image", ico: "image", avif: "image",
  html: "html", htm: "html",
  txt: "text", md: "text", markdown: "text", json: "text", yaml: "text",
  yml: "text", toml: "text", csv: "text", log: "text", xml: "text",
  pdf: "pdf",
  docx: "docx",
  xlsx: "xlsx",
  pptx: "pptx-outline",
  zip: "archive", jar: "archive", apk: "archive", docm: "archive",
  xlsm: "archive", pptm: "archive", epub: "archive",
  rar: "archive-external", "7z": "archive-external", tar: "archive-external",
  gz: "archive-external", tgz: "archive-external", bz2: "archive-external",
  xz: "archive-external", iso: "archive-external", cab: "archive-external",
  psd: "psd",
  stl: "model3d", obj: "model3d", gltf: "model3d", glb: "model3d",
  dxf: "cad2d",
};

/**
 * 明确拒绝的老二进制格式。
 *
 * .doc/.xls/.ppt **不再**映射到 docx/xlsx/pptx —— 老版是完全不同的二进制格式，
 * 当成新版去解析只会得到一堆乱码，然后让用户以为是 Redline 坏了。
 * 理由文案的真相源同样在 Rust（`format::REFUSED`）。
 */
export const REFUSED_EXTENSIONS: Record<string, string> = {
  doc: "旧版 .doc 是二进制格式：Redline 不伪造「无损可编辑」。请保留原件后用 Word/WPS/LibreOffice 另存为 .docx，再执行 inspect/apply。",
  xls: "旧版 .xls 是二进制格式，请先另存为 .xlsx。",
  ppt: "旧版 .ppt 是二进制格式，请先另存为 .pptx。",
};

/** 按文件名判断格式；未知扩展名一律先当文本试着预览（二进制由 TextViewer 自己拒读）。 */
export function detectFormat(fileName: string): RedlineFormat {
  const ext = fileName.split(".").pop()?.toLowerCase() ?? "";
  return EXT_TO_FORMAT[ext] ?? "text";
}

/** 该扩展名是否被明确拒绝；返回可直接展示给用户的理由。 */
export function refusalReason(fileName: string): string | null {
  const ext = fileName.split(".").pop()?.toLowerCase() ?? "";
  return REFUSED_EXTENSIONS[ext] ?? null;
}

/** 未来变现分级预留位——v1 全部 free，先把口碑做出来。表结构已经能承载分级，不等于现在要收费。 */
export const FORMAT_TIERS: Record<RedlineFormat, "free" | "pro"> = {
  image: "free", html: "free", text: "free", pdf: "free", docx: "free",
  xlsx: "free", "pptx-outline": "free", archive: "free",
  "archive-external": "free", psd: "free", model3d: "free", cad2d: "free",
  unsupported: "free",
};

type ViewerLoader = () => Promise<{ default: ComponentType<ViewerProps> }>;

const LOADERS: Record<RedlineFormat, ViewerLoader> = {
  image: () => import("./ImageViewer"),
  html: () => import("./HtmlViewer"),
  text: () => import("./TextViewer"),
  pdf: () => import("./PdfViewer"),
  docx: () => import("./DocxViewer"),
  xlsx: () => import("./SheetViewer"),
  "pptx-outline": () => import("./OfficeOutlineViewer"),
  archive: () => import("./ZipViewer"),
  // rar/7z 还读不了，就别指一个读不了它的 viewer。等外部 7z 后端接上再换成 ZipViewer。
  "archive-external": () => import("./UnsupportedViewer"),
  psd: () => import("./PsdViewer"),
  model3d: () => import("./ModelViewer"),
  cad2d: () => import("./ModelViewer"),
  unsupported: () => import("./UnsupportedViewer"),
};

const cache = new Map<RedlineFormat, ComponentType<ViewerProps>>();

/** 懒加载指定格式的 viewer 组件（React.lazy 包裹，用在 <Suspense> 下）。同格式只包一次。 */
export function loadViewer(format: RedlineFormat): ComponentType<ViewerProps> {
  const hit = cache.get(format);
  if (hit) return hit;
  const Comp = lazy(LOADERS[format]) as unknown as ComponentType<ViewerProps>;
  cache.set(format, Comp);
  return Comp;
}
