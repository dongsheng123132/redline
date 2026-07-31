//! ShadowDoc（影文档）协议的稳定描述信息。
//!
//! 它不是替代 docx/pdf/xlsx 的新主格式，而是与原件 sha256 绑定、可随时重建的语义投影。
//! `document.inspect` 继续是唯一入口；这个模块只定义投影身份，不新增一套重复动作。

use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "redline.shadow-document";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    pub schema: String,
    pub schema_version: u32,
    pub producer: String,
    /// v1 只有 semantic；视觉影子仍由 viewer 从同一原件渲染。
    pub profile: String,
    /// paragraph / sheet / slide / page / document / entry。
    pub granularity: String,
    /// v1 所有投影都诚实标为有损：尚未证明完整样式、对象和隐藏内容都被保留。
    pub lossy: bool,
    /// 显式绑定原件，防止影子 JSON 被脱离 source 后误用。
    pub source_sha256: String,
}

impl Descriptor {
    pub fn for_source(format: &str, source_sha256: &str) -> Self {
        Self {
            schema: SCHEMA.into(),
            schema_version: SCHEMA_VERSION,
            producer: format!("redline-core/{}", env!("CARGO_PKG_VERSION")),
            profile: "semantic".into(),
            granularity: granularity(format).into(),
            lossy: true,
            source_sha256: source_sha256.into(),
        }
    }
}

pub fn granularity(format: &str) -> &'static str {
    match format {
        "docx" => "paragraph",
        "xlsx" => "sheet",
        "pptx-outline" => "slide",
        "pdf" => "page",
        "archive" | "archive-external" => "entry",
        _ => "document",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 重点格式有稳定粒度() {
        assert_eq!(granularity("docx"), "paragraph");
        assert_eq!(granularity("xlsx"), "sheet");
        assert_eq!(granularity("pptx-outline"), "slide");
        assert_eq!(granularity("pdf"), "page");
        assert_eq!(granularity("archive"), "entry");
        assert_eq!(granularity("text"), "document");
    }

    #[test]
    fn 影文档显式绑定原件哈希() {
        let descriptor = Descriptor::for_source("docx", "abc");
        assert_eq!(descriptor.schema, SCHEMA);
        assert_eq!(descriptor.schema_version, 1);
        assert_eq!(descriptor.source_sha256, "abc");
        assert!(descriptor.lossy);
    }
}
