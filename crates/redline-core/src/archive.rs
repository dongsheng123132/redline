//! `archive.list` / `archive.extract` —— 压缩包动作。
//!
//! 这是「取代 WinRAR / 7-Zip」那条线的核心。两条纪律：
//!
//! 1. **只读原包**。解包写目标目录，从不改动、从不删除原压缩包。
//! 2. **zip-slip 必须挡住**。压缩包里的路径是攻击者可控输入，`../../` 和绝对路径
//!    一律拒绝，不做「尽力清洗」——直接拒绝并报出是哪一条，因为清洗过的路径
//!    会让用户以为解出来的东西在他以为的地方。

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};

use crate::error::{RedlineError, Result};
use crate::format::{self, ExtractBackend, FormatSpec};
use crate::inspect::Unit;
use crate::paths::display;

/// 单个压缩包内条目。`format` 是按包内文件名判定的 Redline 格式 id ——
/// 递归预览（点进压缩包直接看里面的 docx）靠它。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry {
    pub path: String,
    pub bytes: u64,
    pub compressed: u64,
    pub is_dir: bool,
    pub format: &'static str,
}

/// inspect 时最多铺多少个条目成 unit。超出部分不静默丢弃，会在 summary 里写明。
const MAX_UNITS: usize = 500;

/// 列出压缩包条目，不解包到磁盘。
pub fn list(path: &str) -> Result<Vec<ArchiveEntry>> {
    let source = crate::inspect::resolve(path)?;
    let spec = spec_for(&source)?;
    match spec.caps.extract {
        ExtractBackend::Native => list_native(&source),
        ExtractBackend::External7z => Err(external_not_wired(spec)),
        ExtractBackend::None => Err(RedlineError::input("not_an_archive", format!("{} 不是压缩包。", source.display()))),
    }
}

fn list_native(source: &Path) -> Result<Vec<ArchiveEntry>> {
    let file = std::fs::File::open(source)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entries = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let item = archive.by_index(index)?;
        let name = item.name().to_string();
        entries.push(ArchiveEntry {
            path: name.clone(),
            bytes: item.size(),
            compressed: item.compressed_size(),
            is_dir: item.is_dir(),
            format: format::detect(&name).id,
        });
    }
    Ok(entries)
}

/// 解包到目标目录。返回实际写出的文件路径。
///
/// **两趟**：先把全部条目路径和覆盖冲突验一遍，一条不合格就整包拒绝；验完才动手写。
/// 单趟边验边写会在遇到恶意条目时留下半截产物 —— 用户看到目录里有东西，
/// 会以为解包成功了。宁可一个字节都不写。
///
/// `dest` 不存在会创建；已存在的同名文件默认拒绝覆盖（`overwrite` 为 false 时）——
/// 「绝不静默覆盖你没创建的东西」。
pub fn extract(path: &str, dest: &str, overwrite: bool) -> Result<Vec<String>> {
    let source = crate::inspect::resolve(path)?;
    let spec = spec_for(&source)?;
    if spec.caps.extract == ExtractBackend::External7z {
        return Err(external_not_wired(spec));
    }
    if spec.caps.extract == ExtractBackend::None {
        return Err(RedlineError::input("not_an_archive", format!("{} 不是压缩包。", source.display())));
    }

    let file = std::fs::File::open(&source)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // ---- 第一趟：只校验，不碰磁盘 ----
    let dest_root = PathBuf::from(dest);
    let dest_root = if dest_root.is_absolute() { dest_root } else { std::env::current_dir()?.join(dest_root) };
    // 目标目录还不存在，canonicalize 不可用；先字面量清掉 ./ 和 ../，免得原样吐给用户
    let dest_root = crate::paths::normalize(dest_root);
    let mut planned: Vec<(usize, PathBuf, bool)> = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let item = archive.by_index(index)?;
        let name = item.name().to_string();
        let relative = safe_relative_path(&name)?;
        let target = dest_root.join(&relative);
        if !item.is_dir() && target.exists() && !overwrite {
            return Err(RedlineError::refused(
                "would_overwrite",
                format!("目标已存在：{}。确认要覆盖请加 --overwrite。", display(&target)),
            )
            .with_details(json!({ "target": display(&target), "entry": name })));
        }
        planned.push((index, target, item.is_dir()));
    }

    // ---- 第二趟：全部合格了才写 ----
    std::fs::create_dir_all(&dest_root)?;
    let mut written = Vec::with_capacity(planned.len());
    for (index, target, is_dir) in planned {
        if is_dir {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut data = Vec::new();
        archive.by_index(index)?.read_to_end(&mut data)?;
        std::fs::write(&target, data)?;
        written.push(display(&target));
    }
    Ok(written)
}

/// 把压缩包内路径校验成安全的相对路径。
///
/// 拒绝：绝对路径、盘符、`..`、Windows 保留的 UNC/前缀。通过的路径保证解不出目标目录。
fn safe_relative_path(name: &str) -> Result<PathBuf> {
    let normalized = name.replace('\\', "/");
    let bytes = normalized.as_bytes();
    let windows_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    // `std::path::Component` follows the build host. A Windows drive or UNC
    // path must therefore be rejected explicitly when CI runs on Linux/macOS.
    if normalized.starts_with('/') || windows_drive {
        return Err(unsafe_path(name));
    }
    let candidate = PathBuf::from(&normalized);
    let mut safe = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(unsafe_path(name));
            }
        }
    }
    if safe.as_os_str().is_empty() {
        return Err(RedlineError::refused("unsafe_archive_path", format!("压缩包条目名非法：{name}")));
    }
    Ok(safe)
}

fn unsafe_path(name: &str) -> RedlineError {
    RedlineError::refused("unsafe_archive_path", format!("压缩包里有会写到目标目录外的路径：{name}。Redline 拒绝解包这一条。"))
        .with_details(json!({ "entry": name }))
}

/// 给 `document.inspect` 用：把压缩包摊成 summary + units。
pub fn summarize(source: &Path, spec: &FormatSpec) -> Result<(Value, Vec<Unit>)> {
    if spec.caps.extract == ExtractBackend::External7z {
        return Err(external_not_wired(spec));
    }
    let entries = list_native(source)?;
    let files: Vec<&ArchiveEntry> = entries.iter().filter(|e| !e.is_dir).collect();
    let total: u64 = files.iter().map(|e| e.bytes).sum();

    let shown = files.len().min(MAX_UNITS);
    let units: Vec<Unit> = files
        .iter()
        .take(MAX_UNITS)
        .enumerate()
        .map(|(i, entry)| {
            Unit::new(
                format!("entry:{}", i + 1),
                entry.path.clone(),
                format!("{} · {} 字节 · 格式 {}", entry.path, entry.bytes, entry.format),
            )
        })
        .collect();

    let mut summary = json!({
        "entries": files.len(),
        "directories": entries.len() - files.len(),
        "uncompressedBytes": total,
    });
    // 有截断就说清楚。静默截断会被读成「已经全看过了」。
    if files.len() > shown {
        summary["truncated"] = json!({
            "shown": shown,
            "omitted": files.len() - shown,
            "hint": "用 `redline archive list` 拿完整清单；快照只铺前 500 条。",
        });
    }
    Ok((summary, units))
}

fn spec_for(source: &Path) -> Result<&'static FormatSpec> {
    let name = source.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let extension = format::extension_of(name);
    format::lookup(&extension).ok_or_else(|| {
        RedlineError::input("unsupported_format", format!("暂不支持 .{extension}")).with_details(json!({ "extension": extension }))
    })
}

/// rar/7z 等格式的解包后端还没接。
///
/// 这里刻意不做「假装支持然后失败」：注册表已经声明了这些扩展名属于 Redline 的
/// 职责范围（预览时能识别、能给出正确提示），但解包能力要等外部 7z 后端接上。
fn external_not_wired(spec: &FormatSpec) -> RedlineError {
    RedlineError::input(
        "extract_backend_missing",
        format!("{} 需要外部解包器（7z），当前版本还没接。zip 系（zip/jar/apk/epub/docm…）已经可用。", spec.label),
    )
    .with_details(json!({ "format": spec.id, "backend": "external_7z" }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 正常相对路径通过() {
        assert_eq!(safe_relative_path("a/b/c.txt").unwrap(), PathBuf::from("a").join("b").join("c.txt"));
        assert_eq!(safe_relative_path("./a.txt").unwrap(), PathBuf::from("a.txt"));
        // zip 规范用 /，但现实里有 \ 的包
        assert_eq!(safe_relative_path("a\\b.txt").unwrap(), PathBuf::from("a").join("b.txt"));
    }

    #[test]
    fn zip_slip_被拒绝() {
        for evil in [
            "../evil.txt",
            "a/../../evil.txt",
            "/etc/passwd",
            "C:\\Windows\\evil.dll",
            "c:drive-relative.txt",
            "\\\\server\\share\\evil.dll",
            "//server/share/evil.dll",
            "..\\evil",
        ] {
            let err = safe_relative_path(evil).unwrap_err();
            assert_eq!(err.code, "unsafe_archive_path", "{evil} 应该被拒绝");
        }
    }

    #[test]
    fn 空条目名被拒绝() {
        assert!(safe_relative_path("").is_err());
        assert!(safe_relative_path("./").is_err());
    }
}
