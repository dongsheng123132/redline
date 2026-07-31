//! `document.inspect` —— 把任意支持格式解析成统一的结构化快照。
//!
//! 快照是 Redline 喂给 AI 的唯一形态：AI 看到的是 `units`（段落 / 页 / 工作表 / 幻灯片），
//! 不是像素。标注层最终也是锚在 unit id 上，所以「人圈的那块」和「AI 读的那段」
//! 说的是同一件事。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{RedlineError, Result};
use crate::format;
use crate::ooxml::{all_text, count_tag, text_by_container, Parts};
use crate::shadow;

/// 快照里的一个可寻址单元。`id` 是 AI 和标注层共用的锚点，必须稳定。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unit {
    pub id: String,
    /// paragraph / sheet / slide / page / document / entry。
    pub kind: String,
    pub label: String,
    pub text: String,
    /// unit 正文的内容身份，用于发现段落/页/工作表已经漂移。
    pub text_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Unit {
    pub(crate) fn new(id: impl Into<String>, label: impl Into<String>, text: impl Into<String>) -> Self {
        let id = id.into();
        let text = text.into();
        let kind = id.split_once(':').map(|(prefix, _)| prefix).unwrap_or("document").to_string();
        let text_sha256 = sha256_hex(text.as_bytes());
        Self { id, kind, label: label.into(), text, text_sha256, note: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub path: String,
    pub extension: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub shadow: shadow::Descriptor,
    pub format: String,
    pub source: SourceInfo,
    pub summary: Value,
    pub units: Vec<Unit>,
}

impl Snapshot {
    pub fn to_json(&self) -> Value {
        json!({
            "shadow": self.shadow,
            "format": self.format,
            "source": self.source,
            "summary": self.summary,
            "units": self.units,
        })
    }
}

pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 把用户给的路径规范化成绝对路径。不存在就直接报输入错误。
pub fn resolve(path: &str) -> Result<PathBuf> {
    let raw = PathBuf::from(path);
    let abs = if raw.is_absolute() { raw } else { std::env::current_dir()?.join(raw) };
    if !abs.is_file() {
        return Err(RedlineError::input("file_not_found", format!("文件不存在：{}", abs.display()))
            .with_details(json!({ "path": abs.display().to_string() })));
    }
    Ok(std::fs::canonicalize(&abs).map(crate::paths::strip_verbatim).unwrap_or(abs))
}

/// 解析一个本地文件成快照。这是 `document.inspect` 的全部实现。
pub fn inspect(path: &str) -> Result<Snapshot> {
    let source = resolve(path)?;
    let file_name = source.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let extension = format::extension_of(file_name);

    if let Some(reason) = format::refusal_reason(&extension) {
        return Err(RedlineError::refused("legacy_binary_format", reason).with_details(json!({ "extension": extension })));
    }

    let spec = format::lookup(&extension).ok_or_else(|| {
        RedlineError::input(
            "unsupported_format",
            format!(
                "暂不支持 .{}；用 `redline formats` 看当前支持的全部格式。",
                if extension.is_empty() { "（无扩展名）".into() } else { extension.clone() }
            ),
        )
        .with_details(json!({ "extension": extension }))
    })?;

    if !spec.caps.inspect {
        return Err(RedlineError::input("format_not_inspectable", format!("{} 只能预览标注，无法解析成结构化快照喂给 AI。", spec.label))
            .with_details(json!({ "format": spec.id })));
    }

    let data = std::fs::read(&source)?;
    let source_info = SourceInfo {
        path: source.display().to_string(),
        extension: extension.clone(),
        bytes: data.len() as u64,
        sha256: sha256_hex(&data),
    };

    let (summary, units) = match spec.id {
        "docx" => docx(&source)?,
        "xlsx" => xlsx(&source)?,
        "pptx-outline" => pptx(&source)?,
        "pdf" => pdf(&source)?,
        "text" | "html" => plain(&data),
        "archive" | "archive-external" => crate::archive::summarize(&source, spec)?,
        other => {
            return Err(RedlineError::internal("inspect_not_wired", format!("格式 {other} 在注册表里声明了 inspect 能力，但核心没接实现")))
        }
    };

    let shadow = shadow::Descriptor::for_source(spec.id, &source_info.sha256);
    Ok(Snapshot { shadow, format: spec.id.to_string(), source: source_info, summary, units })
}

fn docx(path: &Path) -> Result<(Value, Vec<Unit>)> {
    let parts = Parts::read(path)?;
    let document = parts.require_str("word/document.xml")?;
    let paragraphs = text_by_container(&document, "p", "t")?;
    let units =
        paragraphs.iter().enumerate().map(|(i, text)| Unit::new(format!("paragraph:{}", i + 1), format!("段落 {}", i + 1), text)).collect();
    let summary = json!({
        "paragraphs": paragraphs.len(),
        "tables": count_tag(&document, "tbl")?,
        "images": parts.names().filter(|n| n.starts_with("word/media/")).count(),
        "trackedInsertions": count_tag(&document, "ins")?,
        "trackedDeletions": count_tag(&document, "del")?,
    });
    Ok((summary, units))
}

fn pptx(path: &Path) -> Result<(Value, Vec<Unit>)> {
    let parts = Parts::read(path)?;
    let mut slides: Vec<(u32, String)> = parts.names().filter_map(|name| slide_number(name).map(|n| (n, name.to_string()))).collect();
    slides.sort_by_key(|(n, _)| *n);

    let mut units = Vec::with_capacity(slides.len());
    for (index, (_, name)) in slides.iter().enumerate() {
        let xml = parts.require_str(name)?;
        units.push(Unit::new(format!("slide:{}", index + 1), format!("幻灯片 {}", index + 1), all_text(&xml, "t")?));
    }
    Ok((json!({ "slides": units.len() }), units))
}

/// `ppt/slides/slide12.xml` -> 12。只认这一种路径，母版/布局不算幻灯片。
fn slide_number(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("ppt/slides/slide")?.strip_suffix(".xml")?;
    rest.parse().ok()
}

fn xlsx(path: &Path) -> Result<(Value, Vec<Unit>)> {
    let parts = Parts::read(path)?;
    let shared = match parts.get("xl/sharedStrings.xml") {
        Some(raw) => shared_strings(std::str::from_utf8(raw).unwrap_or_default())?,
        None => Vec::new(),
    };
    let workbook = parts.require_str("xl/workbook.xml")?;
    let names = sheet_names(&workbook)?;

    let mut units = Vec::with_capacity(names.len());
    for (index, name) in names.iter().enumerate() {
        let part = format!("xl/worksheets/sheet{}.xml", index + 1);
        let Some(raw) = parts.get(&part) else {
            let mut unit = Unit::new(format!("sheet:{}", index + 1), format!("工作表 {name}"), "");
            unit.note = Some(format!("未找到工作表 XML：{part}"));
            units.push(unit);
            continue;
        };
        let cells = sheet_cells(std::str::from_utf8(raw).unwrap_or_default(), &shared)?;
        units.push(Unit::new(format!("sheet:{}", index + 1), format!("工作表 {name}"), cells.join("\n")));
    }
    Ok((json!({ "sheets": units.len() }), units))
}

fn shared_strings(xml: &str) -> Result<Vec<String>> {
    text_by_container(xml, "si", "t")
}

fn sheet_names(workbook_xml: &str) -> Result<Vec<String>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(workbook_xml);
    let mut names = Vec::new();
    loop {
        match reader.read_event()? {
            Event::Start(e) | Event::Empty(e) => {
                let raw = e.name();
                let local = std::str::from_utf8(raw.as_ref()).unwrap_or("");
                if local.rsplit(':').next() == Some("sheet") {
                    let name = e
                        .try_get_attribute("name")?
                        .map(|a| a.unescape_value().map(|v| v.to_string()))
                        .transpose()?
                        .unwrap_or_else(|| format!("Sheet {}", names.len() + 1));
                    names.push(name);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(names)
}

/// 把工作表铺成 `A1=值 [公式:...]` 一行一个单元格 —— AI 读这个比读 XML 省事得多。
fn sheet_cells(xml: &str, shared: &[String]) -> Result<Vec<String>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    #[derive(PartialEq)]
    enum In {
        None,
        Value,
        Formula,
        InlineText,
    }

    let mut reader = Reader::from_str(xml);
    let mut cells = Vec::new();
    let (mut address, mut kind) = (String::new(), String::new());
    let (mut value, mut formula, mut inline) = (String::new(), String::new(), String::new());
    let mut state = In::None;
    let mut in_cell = false;

    loop {
        match reader.read_event()? {
            Event::Start(e) | Event::Empty(e) => {
                let raw = e.name();
                let full = std::str::from_utf8(raw.as_ref()).unwrap_or("");
                match full.rsplit(':').next().unwrap_or(full) {
                    "c" => {
                        in_cell = true;
                        address = attr(&e, "r")?.unwrap_or_else(|| "?".into());
                        kind = attr(&e, "t")?.unwrap_or_default();
                        value.clear();
                        formula.clear();
                        inline.clear();
                    }
                    "v" if in_cell => state = In::Value,
                    "f" if in_cell => state = In::Formula,
                    "t" if in_cell => state = In::InlineText,
                    _ => {}
                }
            }
            Event::Text(e) => {
                let text = e.unescape()?;
                match state {
                    In::Value => value.push_str(&text),
                    In::Formula => formula.push_str(&text),
                    In::InlineText => inline.push_str(&text),
                    In::None => {}
                }
            }
            Event::End(e) => {
                let raw = e.name();
                let full = std::str::from_utf8(raw.as_ref()).unwrap_or("");
                match full.rsplit(':').next().unwrap_or(full) {
                    "v" | "f" | "t" => state = In::None,
                    "c" => {
                        in_cell = false;
                        // t="s" 表示 v 里存的是 sharedStrings 的下标，不是字面值
                        let display = if kind == "s" {
                            value.trim().parse::<usize>().ok().and_then(|i| shared.get(i).cloned()).unwrap_or_else(|| value.clone())
                        } else if kind == "inlineStr" {
                            inline.clone()
                        } else {
                            value.clone()
                        };
                        if !display.is_empty() || !formula.is_empty() {
                            let mut line = format!("{address}={display}");
                            if !formula.is_empty() {
                                line.push_str(&format!(" [公式:{formula}]"));
                            }
                            cells.push(line);
                        }
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(cells)
}

fn attr(e: &quick_xml::events::BytesStart, name: &str) -> Result<Option<String>> {
    Ok(e.try_get_attribute(name)?.map(|a| a.unescape_value().map(|v| v.to_string())).transpose()?)
}

fn pdf(path: &Path) -> Result<(Value, Vec<Unit>)> {
    let doc = lopdf::Document::load(path).map_err(|e| RedlineError::input("bad_pdf", format!("PDF 解析失败：{e}")))?;
    let pages = doc.get_pages();
    let mut units = Vec::with_capacity(pages.len());
    let mut text_pages = 0usize;
    for (number, _) in pages.iter() {
        // 抽不出文字是常态（扫描件），不是错误 —— 如实标注给 AI，别假装有文本层。
        let text = doc.extract_text(&[*number]).unwrap_or_default();
        let text = text.replace('\u{0}', "");
        let text = text.trim();
        if !text.is_empty() {
            text_pages += 1;
        }
        let mut unit = Unit::new(format!("page:{number}"), format!("第 {number} 页"), text);
        if text.is_empty() {
            unit.note = Some("此页没有文本层（多半是扫描件）；要喂给 AI 需要先做 OCR。".into());
        }
        units.push(unit);
    }
    let summary = json!({
        "pages": units.len(),
        "pagesWithText": text_pages,
        "needsOcr": units.len() - text_pages,
    });
    Ok((summary, units))
}

fn plain(data: &[u8]) -> (Value, Vec<Unit>) {
    let text = String::from_utf8_lossy(data).to_string();
    let lines = text.lines().count();
    let blocks = text_blocks(&text);
    let units = blocks
        .into_iter()
        .enumerate()
        .map(|(index, block)| Unit::new(format!("document:{}", index + 1), text_block_label(&block, index), block))
        .collect::<Vec<_>>();
    (
        json!({
            "lines": lines,
            "chars": text.chars().count(),
            "blocks": units.len(),
            "maxBlockChars": TEXT_BLOCK_MAX_CHARS,
        }),
        units,
    )
}

/// 文本和 Markdown 的确定性语义分块。
///
/// 空行是人的天然段落边界；没有空行的巨大 JSON/日志仍按字符上限切开，避免再次退化成
/// “一个 unit 等于整个文件”。这里只决定 AI/标注锚点，viewer 不得复制这套算法。
const TEXT_BLOCK_MAX_CHARS: usize = 4_000;

fn text_blocks(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut blocks = Vec::new();
    let mut paragraph = String::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            push_bounded_text(&mut blocks, paragraph.trim_end_matches(['\r', '\n']));
            paragraph.clear();
            continue;
        }
        if !paragraph.is_empty() {
            paragraph.push('\n');
        }
        paragraph.push_str(line.trim_end_matches('\r'));
    }
    push_bounded_text(&mut blocks, paragraph.trim_end_matches(['\r', '\n']));

    if blocks.is_empty() {
        blocks.push(String::new());
    }
    blocks
}

fn push_bounded_text(blocks: &mut Vec<String>, text: &str) {
    if text.is_empty() {
        return;
    }

    let mut start = 0usize;
    let mut chars = 0usize;
    for (byte_index, _) in text.char_indices() {
        if chars == TEXT_BLOCK_MAX_CHARS {
            blocks.push(text[start..byte_index].to_string());
            start = byte_index;
            chars = 0;
        }
        chars += 1;
    }
    if start < text.len() {
        blocks.push(text[start..].to_string());
    }
}

fn text_block_label(block: &str, index: usize) -> String {
    let heading = block
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| line.starts_with('#'))
        .map(|line| line.trim_start_matches('#').trim())
        .filter(|line| !line.is_empty())
        .map(|line| line.chars().take(48).collect::<String>());
    match heading {
        Some(heading) => format!("文本块 {} · {heading}", index + 1),
        None => format!("文本块 {}", index + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 幻灯片编号只认标准路径() {
        assert_eq!(slide_number("ppt/slides/slide7.xml"), Some(7));
        assert_eq!(slide_number("ppt/slideLayouts/slideLayout7.xml"), None);
        assert_eq!(slide_number("ppt/slides/_rels/slide7.xml.rels"), None);
    }

    #[test]
    fn 共享字符串下标能还原成字面值() {
        let xml = r#"<worksheet xmlns="x"><sheetData>
            <row><c r="A1" t="s"><v>1</v></c><c r="B1"><v>42</v></c></row>
            <row><c r="A2"><f>SUM(B1:B1)</f><v>42</v></c></row>
        </sheetData></worksheet>"#;
        let shared = vec!["标题".to_string(), "姓名".to_string()];
        let cells = sheet_cells(xml, &shared).unwrap();
        assert_eq!(cells, vec!["A1=姓名", "B1=42", "A2=42 [公式:SUM(B1:B1)]"]);
    }

    #[test]
    fn 空单元格不进快照() {
        let xml = r#"<worksheet><sheetData><row><c r="A1"/><c r="B1"><v>1</v></c></row></sheetData></worksheet>"#;
        assert_eq!(sheet_cells(xml, &[]).unwrap(), vec!["B1=1"]);
    }

    #[test]
    fn 影文档单元带类型和稳定文本哈希() {
        let first = Unit::new("paragraph:7", "段落 7", "同一段文字");
        let second = Unit::new("paragraph:7", "段落 7", "同一段文字");
        assert_eq!(first.kind, "paragraph");
        assert_eq!(first.text_sha256, second.text_sha256);
        assert_eq!(first.text_sha256.len(), 64);
    }

    #[test]
    fn 文本按空行生成确定性块() {
        let (summary, units) = plain("# 标题\n正文\n\n第二段\n\n## 结尾".as_bytes());
        assert_eq!(summary["blocks"], 3);
        assert_eq!(units.len(), 3);
        assert_eq!(units[0].id, "document:1");
        assert_eq!(units[0].kind, "document");
        assert_eq!(units[0].label, "文本块 1 · 标题");
        assert_eq!(units[0].text, "# 标题\n正文");
        assert_eq!(units[2].label, "文本块 3 · 结尾");
    }

    #[test]
    fn 无空行长文本也不会退化成一个超大块() {
        let text = "界".repeat(TEXT_BLOCK_MAX_CHARS * 2 + 7);
        let (_, units) = plain(text.as_bytes());
        assert_eq!(units.len(), 3);
        assert_eq!(units[0].text.chars().count(), TEXT_BLOCK_MAX_CHARS);
        assert_eq!(units[1].text.chars().count(), TEXT_BLOCK_MAX_CHARS);
        assert_eq!(units[2].text.chars().count(), 7);
    }
}
