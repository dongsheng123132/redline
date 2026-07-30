//! Action ID 与输出信封。
//!
//! 影核协议（ActionParity）第 13 条：每个有业务意义的动作给一个稳定 Action ID，
//! 只在这个无界面核心里实现一次。GUI 按钮、CLI 子命令、MCP tool 全是它的调用方，
//! 不是第二份实现。
//!
//! 界面动作（切标签、缩放预览、拖窗口）不在这里，也永远不该进这里。

use crate::error::RedlineError;
use serde_json::{json, Map, Value};

/// 信封版本。字段增减是兼容变更；语义变更才升版本。
pub const ENVELOPE_VERSION: u32 = 1;
pub const TOOL: &str = concat!("redline-core/", env!("CARGO_PKG_VERSION"));

/// 全部业务动作。ID 对外稳定，改名等于破坏兼容。
pub mod id {
    /// 解析本地文件，输出供 AI 消费的结构化快照。只读。
    pub const INSPECT: &str = "document.inspect";
    /// 对比两个同格式文件的快照，输出结构化差异。只读。
    pub const DIFF: &str = "document.diff";
    /// 把显式 patch 写成**新文件**，带原生 Word Track Changes。绝不改原件。
    pub const APPLY: &str = "document.apply-track-changes";
    /// 检查文件能否被解析。只读。
    pub const VERIFY: &str = "document.verify";
    /// 输出格式注册表全文 —— 渲染层靠它对齐，不再自己维护一份扩展名映射。只读。
    pub const FORMATS: &str = "document.formats";
    /// 列出压缩包条目（不解包到磁盘）。只读。
    pub const ARCHIVE_LIST: &str = "archive.list";
    /// 解包到指定目录。写磁盘，但只写目标目录，从不动原压缩包。
    pub const ARCHIVE_EXTRACT: &str = "archive.extract";
    /// 把「标注 + 对应内容 + 源文件」派发给某个 AI agent。agent 写新文件，不动源文件。
    pub const AGENT_DISPATCH: &str = "agent.dispatch";
    /// 可用 agent 清单及其安装状态。只读。
    pub const AGENT_CATALOG: &str = "agent.catalog";
}

/// 统一输出信封。
///
/// 成功与失败都是同一个形状，只有 `ok` 位不同 —— 调用方只需判一个布尔，
/// 不用去猜「这次是 JSON 还是人话」。
pub fn envelope(action_id: &str, payload: Value) -> Value {
    let mut map = Map::new();
    map.insert("ok".into(), Value::Bool(true));
    map.insert("version".into(), ENVELOPE_VERSION.into());
    map.insert("tool".into(), TOOL.into());
    map.insert("action_id".into(), action_id.into());
    match payload {
        Value::Object(fields) => map.extend(fields),
        other => {
            map.insert("result".into(), other);
        }
    }
    Value::Object(map)
}

pub fn error_envelope(action_id: &str, err: &RedlineError) -> Value {
    json!({
        "ok": false,
        "version": ENVELOPE_VERSION,
        "tool": TOOL,
        "action_id": action_id,
        "error": err.to_json(),
    })
}
