//! # redline-core
//!
//! Redline 的**动作核心**：无界面，不知道自己是被 GUI、CLI 还是 MCP 调用的。
//!
//! 影核协议（ActionParity）第 13 条 —— 每个有业务意义的动作只实现一次。
//! [`dispatch`] 是那张唯一的绑定清单：三个界面都从这里进，任何一个界面
//! 想「自己再写一遍逻辑」都会在这张表上露馅。
//!
//! ```no_run
//! let out = redline_core::dispatch("document.inspect", &serde_json::json!({ "path": "方案.docx" }));
//! assert_eq!(out["ok"], true);
//! ```

pub mod action;
pub mod apply;
pub mod archive;
pub mod diff;
pub mod error;
pub mod format;
pub mod inspect;
pub mod ooxml;

use serde_json::{json, Value};

use crate::error::{RedlineError, Result};

pub use crate::action::id as action_id;
pub use crate::error::{ErrorClass, RedlineError as Error};

/// 格式注册表全文 —— 渲染层（TS viewers）靠这个动作对齐，不再自己维护扩展名映射。
pub fn formats() -> Value {
    json!({
        "formats": format::FORMATS,
        "refused": format::REFUSED
            .iter()
            .map(|(ext, reason)| json!({ "extension": ext, "reason": reason }))
            .collect::<Vec<_>>(),
        "fallback": format::FALLBACK,
    })
}

/// 全部动作的绑定清单。新增动作必须同时在这里登记，否则任何界面都调不到它。
pub const ACTIONS: &[(&str, &str)] = &[
    (action::id::INSPECT, "解析本地文件成结构化快照（只读）"),
    (action::id::DIFF, "对比两个同格式文件（只读）"),
    (action::id::APPLY, "把 patch 写成带 Track Changes 的新文件（只有这个动作写文件）"),
    (action::id::VERIFY, "检查文件能否被解析（只读）"),
    (action::id::FORMATS, "输出格式注册表全文（只读）"),
    (action::id::ARCHIVE_LIST, "列出压缩包条目（只读）"),
    (action::id::ARCHIVE_EXTRACT, "解包到目标目录（只写目标目录）"),
];

/// 通用动作入口：给 MCP / Tauri command 这类「按名字调」的调用方用。
///
/// 永远返回一个信封，不会 panic，也不会返回裸错误 —— 调用方只需判 `ok`。
pub fn dispatch(action_id: &str, params: &Value) -> Value {
    match run(action_id, params) {
        Ok(payload) => action::envelope(action_id, payload),
        Err(err) => action::error_envelope(action_id, &err),
    }
}

fn run(action_id: &str, params: &Value) -> Result<Value> {
    match action_id {
        action::id::INSPECT => Ok(inspect::inspect(str_param(params, "path")?)?.to_json()),

        action::id::VERIFY => {
            let snapshot = inspect::inspect(str_param(params, "path")?)?;
            Ok(json!({
                "file": snapshot.source,
                "format": snapshot.format,
                "summary": snapshot.summary,
                "units": snapshot.units.len(),
            }))
        }

        action::id::DIFF => {
            let report = diff::diff_files(str_param(params, "before")?, str_param(params, "after")?)?;
            Ok(serde_json::to_value(report)?)
        }

        action::id::FORMATS => Ok(formats()),

        action::id::ARCHIVE_LIST => {
            let entries = archive::list(str_param(params, "path")?)?;
            Ok(json!({ "count": entries.len(), "entries": entries }))
        }

        action::id::ARCHIVE_EXTRACT => {
            let written = archive::extract(
                str_param(params, "path")?,
                str_param(params, "dest")?,
                params.get("overwrite").and_then(Value::as_bool).unwrap_or(false),
            )?;
            Ok(json!({ "written": written.len(), "files": written }))
        }

        action::id::APPLY => {
            let patch: apply::Patch = serde_json::from_value(
                params
                    .get("patch")
                    .cloned()
                    .ok_or_else(|| RedlineError::input("missing_param", "缺少参数 patch"))?,
            )?;
            let audit = apply::apply_docx(
                str_param(params, "input")?,
                &patch,
                str_param(params, "output")?,
                apply::ApplyOptions {
                    author: params.get("author").and_then(Value::as_str).unwrap_or("AI Redline"),
                    force: params.get("force").and_then(Value::as_bool).unwrap_or(false),
                },
            )?;
            Ok(apply::audit_json(&audit))
        }

        unknown => Err(RedlineError::input(
            "unknown_action",
            format!("未知动作：{unknown}"),
        )
        .with_details(json!({
            "known": ACTIONS.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        }))),
    }
}

fn str_param<'a>(params: &'a Value, name: &str) -> Result<&'a str> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| RedlineError::input("missing_param", format!("缺少字符串参数 {name}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 未知动作返回错误信封而不是_panic() {
        let out = dispatch("document.nope", &json!({}));
        assert_eq!(out["ok"], false);
        assert_eq!(out["error"]["code"], "unknown_action");
        assert_eq!(out["action_id"], "document.nope");
    }

    #[test]
    fn 缺参数报的是输入错误() {
        let out = dispatch(action::id::INSPECT, &json!({}));
        assert_eq!(out["ok"], false);
        assert_eq!(out["error"]["class"], "input");
    }

    #[test]
    fn 格式表动作能跑通且带_viewer_绑定() {
        let out = dispatch(action::id::FORMATS, &json!({}));
        assert_eq!(out["ok"], true);
        let formats = out["formats"].as_array().expect("formats 是数组");
        assert!(formats.iter().any(|f| f["id"] == "docx" && f["viewer"] == "DocxViewer"));
    }

    #[test]
    fn 绑定清单覆盖全部已实现动作() {
        // 清单漏登记的动作，任何界面都不该调得到
        for (id, _) in ACTIONS {
            let out = dispatch(id, &json!({}));
            assert_ne!(out["error"]["code"], "unknown_action", "动作 {id} 没接实现");
        }
    }
}
