//! OOXML（docx/xlsx/pptx）共用的 zip + XML 底座。
//!
//! docx/xlsx/pptx 本质都是「一个 zip 里装一堆 XML」。这里只做最笨也最稳的事：
//! 按原顺序把条目读进内存、按局部标签名抽文本。不引入任何 Office 依赖。

use std::io::{Read, Write};
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{RedlineError, Result};

/// zip 里的一个条目。保留原始顺序很重要 —— OOXML 要求 `[Content_Types].xml`
/// 在最前面，重新打包时按原顺序写回是最省心的做法。
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
    pub is_dir: bool,
}

pub struct Parts {
    pub entries: Vec<Entry>,
}

impl Parts {
    pub fn read(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut entries = Vec::with_capacity(archive.len());
        for index in 0..archive.len() {
            let mut item = archive.by_index(index)?;
            let name = item.name().to_string();
            let is_dir = item.is_dir();
            let mut data = Vec::new();
            if !is_dir {
                item.read_to_end(&mut data)?;
            }
            entries.push(Entry { name, data, is_dir });
        }
        Ok(Self { entries })
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.entries.iter().find(|e| e.name == name).map(|e| e.data.as_slice())
    }

    pub fn set(&mut self, name: &str, data: Vec<u8>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.name == name) {
            entry.data = data;
        } else {
            self.entries.push(Entry { name: name.to_string(), data, is_dir: false });
        }
    }

    /// 取一个必需部件并按 UTF-8 解码。缺件说明这不是一个合法的 OOXML 文件。
    pub fn require_str(&self, name: &str) -> Result<String> {
        let data = self.get(name).ok_or_else(|| RedlineError::input("missing_ooxml_part", format!("缺少 OOXML 部件：{name}")))?;
        String::from_utf8(data.to_vec()).map_err(|_| RedlineError::internal("bad_encoding", format!("部件 {name} 不是合法 UTF-8")))
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.name.as_str())
    }

    /// 按原顺序重新打包写到新路径。
    ///
    /// 只在 `document.apply-track-changes` 里用，且目标路径核心已经保证过不等于原件。
    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let file = std::fs::File::create(path)?;
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for entry in &self.entries {
            if entry.is_dir {
                writer.add_directory(entry.name.clone(), options).map_err(|e| RedlineError::internal("zip_write", e.to_string()))?;
                continue;
            }
            writer.start_file(entry.name.clone(), options).map_err(|e| RedlineError::internal("zip_write", e.to_string()))?;
            writer.write_all(&entry.data)?;
        }
        writer.finish().map_err(|e| RedlineError::internal("zip_write", e.to_string()))?;
        Ok(())
    }
}

/// 按局部标签名把文本切成一段一段。
///
/// `container` 是分段单位（docx 的 `p`、pptx 的 `sp`），`text` 是文本节点
/// （OOXML 里一律是 `t`）。命名空间前缀一律忽略 —— 只认局部名，
/// 避免为了 `w:` / `a:` 前缀差异写两份代码。
pub fn text_by_container(xml: &str, container: &str, text: &str) -> Result<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    let mut depth = 0usize;
    let mut in_text = false;
    let mut current = String::new();
    let mut out = Vec::new();

    loop {
        match reader.read_event()? {
            Event::Start(e) => {
                let name = e.name();
                let local = local_name(name.as_ref());
                if local == container {
                    depth += 1;
                } else if local == text && depth > 0 {
                    in_text = true;
                }
            }
            Event::End(e) => {
                let name = e.name();
                let local = local_name(name.as_ref());
                if local == container && depth > 0 {
                    depth -= 1;
                    if depth == 0 {
                        let trimmed = current.trim();
                        if !trimmed.is_empty() {
                            out.push(trimmed.to_string());
                        }
                        current.clear();
                    }
                } else if local == text {
                    in_text = false;
                }
            }
            Event::Text(e) if in_text => {
                current.push_str(&e.unescape()?);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

/// 抽出整份 XML 里所有指定标签的文本，用空格连起来。
pub fn all_text(xml: &str, text: &str) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    let mut in_text = false;
    let mut pieces: Vec<String> = Vec::new();
    loop {
        match reader.read_event()? {
            Event::Start(e) if local_name(e.name().as_ref()) == text => in_text = true,
            Event::End(e) if local_name(e.name().as_ref()) == text => in_text = false,
            Event::Text(e) if in_text => {
                let piece = e.unescape()?;
                let piece = piece.trim();
                if !piece.is_empty() {
                    pieces.push(piece.to_string());
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(pieces.join(" "))
}

/// 数某个局部标签出现了多少次（起始标签 + 自闭合标签）。
pub fn count_tag(xml: &str, tag: &str) -> Result<usize> {
    let mut reader = Reader::from_str(xml);
    let mut count = 0usize;
    loop {
        match reader.read_event()? {
            Event::Start(e) | Event::Empty(e) if local_name(e.name().as_ref()) == tag => count += 1,
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(count)
}

/// 去掉命名空间前缀，只留局部名。
fn local_name(raw: &[u8]) -> &str {
    let name = std::str::from_utf8(raw).unwrap_or("");
    name.rsplit(':').next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 按容器切分文本并忽略命名空间前缀() {
        let xml = r#"<w:document xmlns:w="x"><w:body>
            <w:p><w:r><w:t>第一段</w:t></w:r><w:r><w:t>接着写</w:t></w:r></w:p>
            <w:p><w:r><w:t>第二段</w:t></w:r></w:p>
            <w:p></w:p>
        </w:body></w:document>"#;
        let got = text_by_container(xml, "p", "t").unwrap();
        assert_eq!(got, vec!["第一段接着写", "第二段"]);
    }

    #[test]
    fn 删除痕迹里的文本不算正文() {
        // w:delText 的局部名是 delText，不是 t，所以不会被算进正文
        let xml = r#"<w:p xmlns:w="x"><w:r><w:delText>删掉的</w:delText></w:r><w:r><w:t>留下的</w:t></w:r></w:p>"#;
        assert_eq!(text_by_container(xml, "p", "t").unwrap(), vec!["留下的"]);
    }

    #[test]
    fn 数标签把自闭合也算上() {
        let xml = r#"<root><tbl/><tbl></tbl><other/></root>"#;
        assert_eq!(count_tag(xml, "tbl").unwrap(), 2);
    }
}
