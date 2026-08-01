//! # redline-core
//!
//! Redline 的**动作核心**：无界面，不知道自己是被 GUI、CLI 还是 MCP 调用的。
//!
//! 影核协议（ActionParity）第 13 条 —— 每个有业务意义的动作只实现一次。
//! [`registry`] 是唯一的 Action-to-handler 注册处：三个界面都从这里进，
//! 任何一个界面想「自己再写一遍逻辑」都会在生成与证据检查里露馅。
//!
//! ```no_run
//! let out = redline_core::dispatch("document.inspect", &serde_json::json!({ "path": "方案.docx" }));
//! assert_eq!(out["ok"], true);
//! ```

pub mod agent;
pub mod apply;
pub mod archive;
pub mod diff;
pub mod error;
pub mod format;
pub mod inspect;
pub mod ooxml;
pub mod paths;
pub mod registry;
pub mod shadow;

use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::{RedlineError, Result};

pub use crate::error::{ErrorClass, RedlineError as Error};
pub use crate::registry::id as action_id;

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

/// 从可执行 Registry 派生的动作清单，不再维护第二份描述表。
pub fn action_catalog() -> Vec<Value> {
    registry::catalog()
}

/// 给 ActionParity 生成器的确定性 Registry Bundle。
pub fn registry_bundle() -> Value {
    registry::bundle()
}

/// The executable Action Registry shared by the Tauri adapter and other
/// machine Surfaces. Cloning this value only clones the `Arc`; handlers and
/// descriptors remain singletons.
pub fn action_registry() -> Arc<action_parity_core::Registry> {
    registry::get()
}

/// Dispatch through the executable Registry so machine Surfaces receive the
/// standard ActionParity envelope and a traceable execution ID.
pub fn dispatch_surface(surface: &str, action_id: &str, params: Value, confirmed: bool, execution_id: Option<String>) -> Value {
    dispatch_request(Some(surface), action_id, params, confirmed, execution_id)
}

fn dispatch_request(surface: Option<&str>, action_id: &str, params: Value, confirmed: bool, execution_id: Option<String>) -> Value {
    serde_json::to_value(registry::get().dispatch(action_parity_core::DispatchRequest {
        action_id: action_id.into(),
        input: params,
        confirmed,
        execution_id,
        surface: surface.map(str::to_string),
    }))
    .expect("ActionParity execution envelopes are always serializable")
}

/// 通用动作入口：给 MCP / Tauri command 这类「按名字调」的调用方用。
///
/// 永远返回一个信封，不会 panic，也不会返回裸错误 —— 调用方只需判 `ok`。
pub fn dispatch(action_id: &str, params: &Value) -> Value {
    dispatch_request(None, action_id, params.clone(), true, None)
}

pub(crate) fn inspect_action(params: &Value) -> Result<Value> {
    Ok(inspect::inspect(str_param(params, "path")?)?.to_json())
}

pub(crate) fn verify_action(params: &Value) -> Result<Value> {
    let snapshot = inspect::inspect(str_param(params, "path")?)?;
    Ok(json!({
        "file": snapshot.source,
        "format": snapshot.format,
        "summary": snapshot.summary,
        "units": snapshot.units.len(),
    }))
}

pub(crate) fn diff_action(params: &Value) -> Result<Value> {
    Ok(serde_json::to_value(diff::diff_files(str_param(params, "before")?, str_param(params, "after")?)?)?)
}

pub(crate) fn formats_action(_params: &Value) -> Result<Value> {
    Ok(formats())
}

pub(crate) fn archive_list_action(params: &Value) -> Result<Value> {
    let entries = archive::list(str_param(params, "path")?)?;
    Ok(json!({ "count": entries.len(), "entries": entries }))
}

pub(crate) fn archive_extract_action(params: &Value) -> Result<Value> {
    let written = archive::extract(
        str_param(params, "path")?,
        str_param(params, "dest")?,
        params.get("overwrite").and_then(Value::as_bool).unwrap_or(false),
    )?;
    Ok(json!({ "written": written.len(), "files": written }))
}

pub(crate) fn agent_catalog_action(_params: &Value) -> Result<Value> {
    Ok(agent::catalog())
}

pub(crate) fn agent_dispatch_action(params: &Value) -> Result<Value> {
    let annotations: Vec<agent::Annotation> = params.get("annotations").map(parse_annotations).transpose()?.unwrap_or_default();
    let report = agent::dispatch(agent::DispatchRequest {
        agent: str_param(params, "agent")?,
        source: str_param(params, "source")?,
        expected_source_sha256: params.get("expectedSourceSha256").and_then(Value::as_str).map(str::to_string),
        output: str_param(params, "output")?,
        annotations,
        instruction: params.get("instruction").and_then(Value::as_str).map(str::to_string),
        cwd: params.get("cwd").and_then(Value::as_str).map(str::to_string),
        timeout_secs: params.get("timeoutSecs").and_then(Value::as_u64).unwrap_or(0),
        dry_run: params.get("dryRun").and_then(Value::as_bool).unwrap_or(false),
    })?;
    Ok(serde_json::to_value(report)?)
}

pub(crate) fn apply_action(params: &Value) -> Result<Value> {
    let patch: apply::Patch =
        serde_json::from_value(params.get("patch").cloned().ok_or_else(|| RedlineError::input("missing_param", "缺少参数 patch"))?)?;
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

/// 标注可以只给 `{unitId, note}`，正文由核心从快照里补 —— 界面不必自己去读文档。
fn parse_annotations(value: &Value) -> Result<Vec<agent::Annotation>> {
    let items = value.as_array().ok_or_else(|| RedlineError::input("bad_annotations", "annotations 必须是数组"))?;
    items
        .iter()
        .map(|item| {
            let note =
                item.get("note").and_then(Value::as_str).ok_or_else(|| RedlineError::input("bad_annotations", "每条标注都要有 note"))?;
            Ok(agent::Annotation {
                unit_id: item.get("unitId").and_then(Value::as_str).map(str::to_string),
                note: note.to_string(),
                unit_text: item.get("unitText").and_then(Value::as_str).map(str::to_string),
            })
        })
        .collect()
}

fn str_param<'a>(params: &'a Value, name: &str) -> Result<&'a str> {
    params.get(name).and_then(Value::as_str).ok_or_else(|| RedlineError::input("missing_param", format!("缺少字符串参数 {name}")))
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
        let out = dispatch(action_id::INSPECT, &json!({}));
        assert_eq!(out["ok"], false);
        assert_eq!(out["error"]["class"], "input");
    }

    #[test]
    fn 格式表动作能跑通且带_viewer_绑定() {
        let out = dispatch(action_id::FORMATS, &json!({}));
        assert_eq!(out["ok"], true);
        let formats = out["result"]["formats"].as_array().expect("formats 是数组");
        assert!(formats.iter().any(|f| f["id"] == "docx" && f["viewer"] == "DocxViewer"));
    }

    #[test]
    fn inspect_信封包含与原件绑定的影文档() {
        let unique = format!(
            "redline-shadow-envelope-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        );
        let path = std::env::temp_dir().join(format!("{unique}.txt"));
        std::fs::write(&path, "影文档").unwrap();
        let out = dispatch(action_id::INSPECT, &json!({ "path": path.to_string_lossy() }));
        let _ = std::fs::remove_file(&path);

        assert_eq!(out["ok"], true);
        assert_eq!(out["result"]["shadow"]["schema"], shadow::SCHEMA);
        assert_eq!(out["result"]["shadow"]["schemaVersion"], 1);
        assert_eq!(out["result"]["shadow"]["sourceSha256"], out["result"]["source"]["sha256"]);
        assert_eq!(out["result"]["units"][0]["kind"], "document");
        assert_eq!(out["result"]["units"][0]["textSha256"].as_str().unwrap().len(), 64);
    }

    #[test]
    fn 绑定清单覆盖全部已实现动作() {
        // 清单漏登记的动作，任何界面都不该调得到
        for item in action_catalog() {
            let id = item["id"].as_str().unwrap();
            let out = dispatch(id, &json!({}));
            assert_ne!(out["error"]["code"], "unknown_action", "动作 {id} 没接实现");
        }
    }
}
