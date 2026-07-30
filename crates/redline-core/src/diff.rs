//! `document.diff` —— 两份同格式文件的结构化差异。
//!
//! 按 unit id 对齐，不按行对齐。这样「第 3 段改了」在 docx 里就是 `paragraph:3`，
//! 在 pptx 里就是 `slide:3`，标注层和 AI 拿到的是同一个锚点。

use serde::Serialize;
use serde_json::json;

use crate::error::{RedlineError, Result};
use crate::inspect::{self, Snapshot};

#[derive(Debug, Clone, Serialize)]
pub struct Change {
    pub id: String,
    pub label: String,
    pub before: String,
    pub after: String,
    /// `added` / `removed` / `modified` —— 让调用方不用自己比空串。
    pub kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffReport {
    pub format: String,
    pub before: inspect::SourceInfo,
    pub after: inspect::SourceInfo,
    pub summary: serde_json::Value,
    pub changes: Vec<Change>,
}

pub fn diff_files(before_path: &str, after_path: &str) -> Result<DiffReport> {
    diff_snapshots(&inspect::inspect(before_path)?, &inspect::inspect(after_path)?)
}

pub fn diff_snapshots(before: &Snapshot, after: &Snapshot) -> Result<DiffReport> {
    if before.format != after.format {
        return Err(RedlineError::input(
            "format_mismatch",
            format!("只能对比同一格式的文件：{} vs {}", before.format, after.format),
        )
        .with_details(json!({ "before": before.format, "after": after.format })));
    }

    let mut ids: Vec<&str> = before
        .units
        .iter()
        .chain(after.units.iter())
        .map(|u| u.id.as_str())
        .collect();
    ids.sort_unstable_by(|a, b| natural_order(a, b));
    ids.dedup();

    let mut changes = Vec::new();
    for id in &ids {
        let old = before.units.iter().find(|u| u.id == *id);
        let new = after.units.iter().find(|u| u.id == *id);
        let old_text = old.map(|u| u.text.as_str()).unwrap_or_default();
        let new_text = new.map(|u| u.text.as_str()).unwrap_or_default();
        if old_text == new_text {
            continue;
        }
        let kind = match (old.is_some(), new.is_some()) {
            (false, true) => "added",
            (true, false) => "removed",
            _ => "modified",
        };
        changes.push(Change {
            id: (*id).to_string(),
            label: new.or(old).map(|u| u.label.clone()).unwrap_or_else(|| (*id).to_string()),
            before: old_text.to_string(),
            after: new_text.to_string(),
            kind,
        });
    }

    let summary = json!({
        "totalUnits": ids.len(),
        "changedUnits": changes.len(),
        "unchangedUnits": ids.len() - changes.len(),
    });
    Ok(DiffReport {
        format: before.format.clone(),
        before: before.source.clone(),
        after: after.source.clone(),
        summary,
        changes,
    })
}

/// `paragraph:2` 要排在 `paragraph:10` 前面。纯字典序会排成 10 在 2 前面，
/// 让 diff 输出读起来像乱的。
fn natural_order(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| -> (String, u64) {
        match s.rsplit_once(':') {
            Some((prefix, num)) => match num.parse::<u64>() {
                Ok(n) => (prefix.to_string(), n),
                Err(_) => (s.to_string(), 0),
            },
            None => (s.to_string(), 0),
        }
    };
    let (pa, na) = split(a);
    let (pb, nb) = split(b);
    pa.cmp(&pb).then(na.cmp(&nb))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(format: &str, units: &[(&str, &str)]) -> Snapshot {
        Snapshot {
            format: format.into(),
            source: inspect::SourceInfo {
                path: "x".into(),
                extension: format.into(),
                bytes: 0,
                sha256: "0".into(),
            },
            summary: json!({}),
            units: units
                .iter()
                .map(|(id, text)| inspect::Unit {
                    id: (*id).into(),
                    label: (*id).into(),
                    text: (*text).into(),
                    note: None,
                })
                .collect(),
        }
    }

    #[test]
    fn 只报有变化的单元并标出类型() {
        let before = snapshot("docx", &[("paragraph:1", "同"), ("paragraph:2", "旧")]);
        let after = snapshot("docx", &[("paragraph:1", "同"), ("paragraph:2", "新"), ("paragraph:3", "多的")]);
        let report = diff_snapshots(&before, &after).unwrap();
        assert_eq!(report.changes.len(), 2);
        assert_eq!(report.changes[0].kind, "modified");
        assert_eq!(report.changes[1].kind, "added");
        assert_eq!(report.summary["unchangedUnits"], 1);
    }

    #[test]
    fn 单元编号按数值排序而不是字典序() {
        let before = snapshot("docx", &[("paragraph:2", "a"), ("paragraph:10", "b")]);
        let after = snapshot("docx", &[("paragraph:2", "A"), ("paragraph:10", "B")]);
        let report = diff_snapshots(&before, &after).unwrap();
        assert_eq!(report.changes[0].id, "paragraph:2");
        assert_eq!(report.changes[1].id, "paragraph:10");
    }

    #[test]
    fn 跨格式对比被拒绝() {
        let err = diff_snapshots(&snapshot("docx", &[]), &snapshot("pdf", &[])).unwrap_err();
        assert_eq!(err.code, "format_mismatch");
    }
}
