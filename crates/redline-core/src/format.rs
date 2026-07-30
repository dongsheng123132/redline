//! 格式注册表 —— **单一真相源**。
//!
//! 「Redline 支持哪些格式、某个扩展名算什么格式、该格式能不能标注/能不能改写/
//! 用哪个 viewer 渲染」这些事实，全项目只在这张表里存一份。
//!
//! 在这张表出现之前，同样的事实存在两份：`bin/redline.py` 里一份（给 AI 抽结构用）、
//! `packages/redline-core/viewers/registry.ts` 里一份（给人渲染用），两边已经开始漂移
//! （TS 认 psd/dxf/stl，Python 不认；Python 明确拒绝 .doc，TS 把 .doc 当 docx 试着渲染）。
//! 现在 Rust 这份是唯一权威，TS 那份降级成渲染层，靠 `document.formats` 动作对齐。

use serde::Serialize;

/// 变现分级预留位 —— v1 全部 free，表结构先能承载分级，不等于现在要收费。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
}

/// 压缩包的解包后端。分开是因为 license：
/// - `Native`：纯 Rust（zip crate），无任何传染风险。
/// - `External7z`：进程外调用 7z 可执行文件。RAR 解压代码用的是 unRAR license
///   （禁止用于重建 RAR 压缩器），**只有进程外调用才不把它拖进我们的二进制**，
///   跟本项目处理 LibreOffice 的老办法一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractBackend {
    None,
    Native,
    External7z,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Caps {
    /// 能不能解析出结构化快照喂给 AI（`document.inspect`）。
    pub inspect: bool,
    /// 能不能做结构化 diff（`document.diff`）。
    pub diff: bool,
    /// 能不能生成带修订痕迹的新文件（`document.apply-track-changes`）。
    pub apply: bool,
    /// 能不能在预览上叠加标注层。
    pub annotate: bool,
    /// 解包后端。
    pub extract: ExtractBackend,
}

impl Caps {
    const fn viewable() -> Self {
        Self { inspect: false, diff: false, apply: false, annotate: true, extract: ExtractBackend::None }
    }
    const fn readable() -> Self {
        Self { inspect: true, diff: true, apply: false, annotate: true, extract: ExtractBackend::None }
    }
    const fn writable() -> Self {
        Self { inspect: true, diff: true, apply: true, annotate: true, extract: ExtractBackend::None }
    }
    const fn archive(backend: ExtractBackend) -> Self {
        Self { inspect: true, diff: false, apply: false, annotate: false, extract: backend }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct FormatSpec {
    /// 稳定格式 id，跨 CLI / GUI / MCP 一致。
    pub id: &'static str,
    pub label: &'static str,
    pub extensions: &'static [&'static str],
    /// 渲染层（TS）对应的 viewer 模块名。核心自己不渲染，只负责说清「该用哪个」。
    pub viewer: &'static str,
    pub tier: Tier,
    pub caps: Caps,
}

/// 全量格式表。新增格式**只改这里**。
pub const FORMATS: &[FormatSpec] = &[
    FormatSpec {
        id: "image",
        label: "图片",
        extensions: &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif"],
        viewer: "ImageViewer",
        tier: Tier::Free,
        caps: Caps::viewable(),
    },
    FormatSpec {
        id: "html",
        label: "网页",
        extensions: &["html", "htm"],
        viewer: "HtmlViewer",
        tier: Tier::Free,
        caps: Caps::readable(),
    },
    FormatSpec {
        id: "text",
        label: "纯文本",
        extensions: &["txt", "md", "markdown", "json", "yaml", "yml", "toml", "csv", "log", "xml"],
        viewer: "TextViewer",
        tier: Tier::Free,
        caps: Caps::readable(),
    },
    FormatSpec {
        id: "pdf",
        label: "PDF",
        extensions: &["pdf"],
        viewer: "PdfViewer",
        tier: Tier::Free,
        caps: Caps::readable(),
    },
    FormatSpec {
        id: "docx",
        label: "Word 文档",
        extensions: &["docx"],
        viewer: "DocxViewer",
        tier: Tier::Free,
        // 唯一支持 apply 的格式：Word 有原生 Track Changes 可以承载修订痕迹。
        caps: Caps::writable(),
    },
    FormatSpec {
        id: "xlsx",
        label: "Excel 表格",
        extensions: &["xlsx"],
        viewer: "SheetViewer",
        tier: Tier::Free,
        caps: Caps::readable(),
    },
    FormatSpec {
        id: "pptx-outline",
        label: "PPT 大纲",
        extensions: &["pptx"],
        viewer: "OfficeOutlineViewer",
        tier: Tier::Free,
        caps: Caps::readable(),
    },
    FormatSpec {
        id: "archive",
        label: "压缩包",
        extensions: &["zip", "jar", "apk", "docm", "xlsm", "pptm", "epub"],
        viewer: "ZipViewer",
        tier: Tier::Free,
        caps: Caps::archive(ExtractBackend::Native),
    },
    FormatSpec {
        id: "archive-external",
        label: "压缩包（需外部解包器）",
        extensions: &["rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "iso", "cab"],
        // 还读不了，就别指一个读不了它的 viewer。等 7z 后端接上再换成 ZipViewer。
        viewer: "UnsupportedViewer",
        tier: Tier::Free,
        caps: Caps::archive(ExtractBackend::External7z),
    },
    FormatSpec {
        id: "psd",
        label: "Photoshop 文档",
        extensions: &["psd"],
        viewer: "PsdViewer",
        tier: Tier::Free,
        caps: Caps::viewable(),
    },
    FormatSpec {
        id: "model3d",
        label: "3D 模型",
        extensions: &["stl", "obj", "gltf", "glb"],
        viewer: "ModelViewer",
        tier: Tier::Free,
        caps: Caps::viewable(),
    },
    FormatSpec {
        id: "cad2d",
        label: "CAD 图纸",
        extensions: &["dxf"],
        viewer: "ModelViewer",
        tier: Tier::Free,
        caps: Caps::viewable(),
    },
];

/// 明确拒绝的老格式 —— 拒绝理由要能原样念给用户听。
///
/// 这些格式技术上能勉强读出点东西，但读出来的东西不足以支撑「改完还能还原」，
/// 与其伪装成"无损可编辑"，不如直说：先另存为新格式。
pub const REFUSED: &[(&str, &str)] = &[
    ("doc", "旧版 .doc 是二进制格式：Redline 不伪造「无损可编辑」。请保留原件后用 Word/WPS/LibreOffice 另存为 .docx，再执行 inspect/apply。"),
    ("xls", "旧版 .xls 是二进制格式，请先另存为 .xlsx。"),
    ("ppt", "旧版 .ppt 是二进制格式，请先另存为 .pptx。"),
];

/// 未知扩展名的兜底格式：先当文本试着预览（二进制内容由 viewer 自己拒读）。
pub const FALLBACK: &FormatSpec = &FormatSpec {
    id: "unsupported",
    label: "未识别",
    extensions: &[],
    viewer: "UnsupportedViewer",
    tier: Tier::Free,
    caps: Caps { inspect: false, diff: false, apply: false, annotate: true, extract: ExtractBackend::None },
};

/// 从文件名取小写扩展名（不含点）。没有扩展名返回空串。
pub fn extension_of(file_name: &str) -> String {
    file_name.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default()
}

/// 按扩展名查格式。未知扩展名返回 `None`（调用方自行决定是兜底还是报错）。
pub fn lookup(extension: &str) -> Option<&'static FormatSpec> {
    let ext = extension.trim_start_matches('.').to_ascii_lowercase();
    FORMATS.iter().find(|spec| spec.extensions.contains(&ext.as_str()))
}

/// 按文件名查格式，未知一律落到 `FALLBACK`。
pub fn detect(file_name: &str) -> &'static FormatSpec {
    lookup(&extension_of(file_name)).unwrap_or(FALLBACK)
}

/// 该扩展名是否被明确拒绝；返回可直接展示给用户的理由。
pub fn refusal_reason(extension: &str) -> Option<&'static str> {
    let ext = extension.trim_start_matches('.').to_ascii_lowercase();
    REFUSED.iter().find(|(e, _)| *e == ext).map(|(_, reason)| *reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 扩展名映射到格式() {
        assert_eq!(detect("方案.docx").id, "docx");
        assert_eq!(detect("A.PDF").id, "pdf");
        assert_eq!(detect("包.rar").id, "archive-external");
        assert_eq!(detect("无扩展名").id, "unsupported");
    }

    #[test]
    fn 老格式被明确拒绝而不是当成新格式() {
        assert!(refusal_reason("doc").is_some());
        // .doc 不能落到 docx 上——TS 那份注册表当年就是这么错的
        assert!(lookup("doc").is_none());
    }

    #[test]
    fn 扩展名在全表内不重复() {
        let mut seen = std::collections::HashSet::new();
        for spec in FORMATS {
            for ext in spec.extensions {
                assert!(seen.insert(*ext), "扩展名 {ext} 在格式表里出现了两次");
            }
        }
        for (ext, _) in REFUSED {
            assert!(!seen.contains(ext), "扩展名 {ext} 既在支持表又在拒绝表");
        }
    }

    #[test]
    fn 只有_docx_支持_apply() {
        for spec in FORMATS {
            assert_eq!(spec.caps.apply, spec.id == "docx", "格式 {} 的 apply 能力不对", spec.id);
        }
    }
}
