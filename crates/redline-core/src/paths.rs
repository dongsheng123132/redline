//! 路径对外呈现的唯一规则。
//!
//! Windows 的 `canonicalize` 会返回 `\\?\C:\...` 这种长路径前缀。它在 Rust 内部
//! 没问题，但一旦进了 JSON 就会被下游当成路径的一部分：agent 拿去拼命令行、
//! 前端拿去拼 URL、用户直接复制粘贴 —— 很多工具认不了这个前缀。
//!
//! 所以核心对外吐的每一条路径都要过这里。**只此一份**，别在各模块里各写一遍。

use std::path::{Path, PathBuf};

/// 去掉 `\\?\` 前缀。UNC 网络路径（`\\?\UNC\server\share`）要补回开头的 `\\`。
pub fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(match rest.strip_prefix("UNC\\") {
            Some(unc) => format!(r"\\{unc}"),
            None => rest.to_string(),
        }),
        None => path,
    }
}

/// 路径转成对外可用的字符串。
pub fn display(path: &Path) -> String {
    strip_verbatim(path.to_path_buf()).to_string_lossy().to_string()
}

/// 纯字面量地清掉 `.` 和 `..`，**不碰文件系统**。
///
/// 给「目标目录还不存在，没法 canonicalize，但又不想把 `./` 原样吐给用户」的场景用。
/// 注意：这是字面量处理，遇到软链接的语义跟 `canonicalize` 不同 ——
/// 只用来整理输出路径的观感，不能拿来做安全判断（安全判断在 archive 那边按条目做）。
pub fn normalize(path: PathBuf) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // 弹不掉（开头就是 ..）就原样保留，别悄悄改变语义
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 长路径前缀被剥掉() {
        assert_eq!(strip_verbatim(PathBuf::from(r"\\?\C:\a\b.docx")), PathBuf::from(r"C:\a\b.docx"));
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\UNC\srv\share\a.docx")),
            PathBuf::from(r"\\srv\share\a.docx")
        );
        // 本来就没有前缀的路径原样返回
        assert_eq!(strip_verbatim(PathBuf::from("/tmp/a.docx")), PathBuf::from("/tmp/a.docx"));
    }

    #[test]
    fn 点和上级目录被字面量清掉() {
        assert_eq!(normalize(PathBuf::from("a/./b")), PathBuf::from("a").join("b"));
        assert_eq!(normalize(PathBuf::from("a/b/../c")), PathBuf::from("a").join("c"));
        assert_eq!(normalize(PathBuf::from("./")), PathBuf::from("."));
        // 弹不掉的 .. 保留，不悄悄改变语义
        assert_eq!(normalize(PathBuf::from("../x")), PathBuf::from("..").join("x"));
    }
}
