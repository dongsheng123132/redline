/**
 * 统一文档模型 —— 格式差异在这一层被抹平。
 *
 * PDF 的页、PPTX 的大纲条目、PSD 的图层、XLSX 的 sheet、ZIP 的条目、3D 模型的场景……
 * 对外都长成同一种「可标注单元」（RedlineUnit）。viewer 和 AnnotationLayer 只认这个模型，
 * 不关心具体格式怎么解析出来的。
 *
 * 标注坐标一律按 unit.renderSize 换算，跟屏幕缩放/DPI 无关，这样标注跨会话、跨窗口大小
 * 重新打开也能对得上原位置。
 */

export type RedlineFormat =
  | "image"
  | "html"
  | "text"
  | "pdf"
  | "docx"
  | "xlsx"
  | "pptx-outline"
  | "zip"
  | "psd"
  | "model3d"
  | "cad2d"
  | "unsupported";

/** 文档内一个可标注单元（页/图层/sheet/条目/场景……）。 */
export interface RedlineUnit {
  /** 单元在文档内的稳定标识，同一文档重开后要保持一致，标注才能对得上号。 */
  id: string;
  /** 展示用序号，从 0 开始。 */
  index: number;
  /** 展示用标签，例如「第 3 页」「图层：背景」「Sheet1」。 */
  label: string;
  /** 标注 bbox 按这个坐标系换算；没有明确二维画布的格式（如纯文本）可留空。 */
  renderSize?: { width: number; height: number };
  /** 供 AI 读取的文字内容（解析/OCR 出来的），随标注一起导出给 agent。 */
  text?: string;
}

/** 一份被 Redline 打开的文档。只读——Redline 从不回写 sourcePath 指向的文件。 */
export interface RedlineDocument {
  docId: string;
  sourcePath: string;
  format: RedlineFormat;
  /** 大部分格式只有一个 unit（图片/html/text/docx）；PDF/PSD/XLSX/ZIP/3D 是多 unit。 */
  units: RedlineUnit[];
  /** 解析/渲染中途的非致命提示，比如「STEP/IGES 暂不支持」「pptx 仅提取大纲，未逐页渲染」。 */
  notes?: string[];
}

export type RedlineAnnotationKind = "rect" | "arrow" | "freehand" | "note";

/**
 * 一条标注。落在某个 unit 上，产出结构化数据而非像素涂鸦。
 *
 * bbox/points 一律是相对 unit 当前显示框的**分数坐标（0~1）**，不是像素——
 * 这样窗口缩放、DPI 变化、甚至换成全分辨率截图时都不用做换算/重新定位，
 * 拿分数坐标乘以任意分辨率（unit.renderSize 或截图实际尺寸）就是对应像素矩形。
 */
export interface RedlineAnnotation {
  id: string;
  kind: RedlineAnnotationKind;
  unitId: string;
  /** rect / arrow 用起止 bbox（分数坐标 0~1）；note 只用左上角一点，宽高记 0。 */
  bbox?: { x: number; y: number; width: number; height: number };
  /** freehand 画笔轨迹点（分数坐标 0~1）。 */
  points?: { x: number; y: number }[];
  note: string;
  createdAt: number;
}

/** 标注导出给 agent 消费的结构——带上 unit 的展示标签和文字内容，agent 不用反查文档模型。 */
export interface RedlineAnnotationExport {
  docPath: string;
  format: RedlineFormat;
  annotations: Array<RedlineAnnotation & { unitLabel: string; unitText?: string }>;
}

/** 把文档 + 标注列表拼成 agent 可读的导出结构。 */
export function exportAnnotations(
  doc: RedlineDocument,
  annotations: RedlineAnnotation[],
): RedlineAnnotationExport {
  const unitById = new Map(doc.units.map((u) => [u.id, u]));
  return {
    docPath: doc.sourcePath,
    format: doc.format,
    annotations: annotations.map((a) => {
      const unit = unitById.get(a.unitId);
      return { ...a, unitLabel: unit?.label ?? a.unitId, unitText: unit?.text };
    }),
  };
}

/** 把导出结构渲染成一段人类/agent 都能读的纯文本，供「发给终端」按钮直接写入 PTY。 */
export function formatAnnotationsAsText(exported: RedlineAnnotationExport, doc: RedlineDocument): string {
  const unitById = new Map(doc.units.map((u) => [u.id, u]));
  const lines = [`[Redline 标注] ${exported.docPath}（${exported.format}）`];
  exported.annotations.forEach((a, i) => {
    const renderSize = unitById.get(a.unitId)?.renderSize;
    const where = a.bbox
      ? renderSize
        ? `约位于(${Math.round(a.bbox.x * renderSize.width)},${Math.round(a.bbox.y * renderSize.height)}) 大小${Math.round(a.bbox.width * renderSize.width)}x${Math.round(a.bbox.height * renderSize.height)}px`
        : `相对位置(${a.bbox.x.toFixed(2)},${a.bbox.y.toFixed(2)}) 大小${a.bbox.width.toFixed(2)}x${a.bbox.height.toFixed(2)}`
      : a.points
        ? `画笔轨迹(${a.points.length}点)`
        : "";
    lines.push(`${i + 1}. [${a.unitLabel}] ${where} —— ${a.note}`);
    if (a.unitText) {
      const snippet = a.unitText.length > 200 ? `${a.unitText.slice(0, 200)}…` : a.unitText;
      lines.push(`   该处内容：${snippet}`);
    }
  });
  return lines.join("\n");
}
