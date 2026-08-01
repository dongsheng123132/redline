//! Redline 桌面壳的后端 —— 动作核心的 **GUI 面**。
//!
//! 这里只有一扇门通向业务：生成的 `action_parity_call`。上游薄 Adapter
//! 把请求原样转给 `redline_core` 的共享 Registry，跟 CLI 的 `redline call`、
//! 将来 MCP 的 tool 调的是同一个入口、同一份实现。
//!
//! 除此之外的 command 全是**宿主能力**（读字节给 viewer、存标注、开文件选择器），
//! 不是业务动作 —— 它们换个宿主就要重写一遍，而业务动作永远只有一份。

use std::sync::Mutex;

use action_parity_tauri::{tauri_command, TauriAdapter};
use serde_json::{json, Value};
use tauri::Manager;

// 上游 Adapter 生成唯一业务 command：只反序列化请求并转发到共享 Registry。
tauri_command!(action_parity_call);

/// 全部可用动作，给界面自查用（也让「GUI 到底绑了哪些动作」可被机器检查）。
#[tauri::command]
fn redline_actions() -> Value {
    json!({
        "actions": redline_core::action_catalog(),
    })
}

/// 读文件原始字节给 viewer 渲染。宿主能力，不是业务动作。
#[tauri::command]
fn read_file_bytes(path: String) -> Result<Vec<u8>, String> {
    std::fs::read(&path).map_err(|err| format!("读不到 {path}：{err}"))
}

/// 命令行/文件关联带进来的待打开文件。
struct InitialFile(Option<String>);

/// 前端启动时取一次。取不到就是空白工作台。
#[tauri::command]
fn initial_file(state: tauri::State<'_, InitialFile>) -> Option<String> {
    state.0.clone()
}

/// 标注持久化。存成 app data 目录下的一个 JSON，宿主自己的事。
struct AnnotationStore {
    file: std::path::PathBuf,
    data: Mutex<serde_json::Map<String, Value>>,
}

impl AnnotationStore {
    fn load(file: std::path::PathBuf) -> Self {
        let data = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Map<String, Value>>(&text).ok())
            .unwrap_or_default();
        Self { file, data: Mutex::new(data) }
    }

    fn flush(&self, data: &serde_json::Map<String, Value>) {
        if let Some(parent) = self.file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // 先写临时文件再改名 —— 半截 JSON 会让下次启动丢掉全部标注
        let temporary = self.file.with_extension("json.tmp");
        if serde_json::to_string_pretty(data).ok().and_then(|text| std::fs::write(&temporary, text).ok()).is_some() {
            let _ = std::fs::rename(&temporary, &self.file);
        }
    }
}

#[tauri::command]
fn kv_get(store: tauri::State<'_, AnnotationStore>, key: String) -> Option<String> {
    store.data.lock().ok()?.get(&key).and_then(Value::as_str).map(str::to_string)
}

#[tauri::command]
fn kv_set(store: tauri::State<'_, AnnotationStore>, key: String, value: Option<String>) {
    let Ok(mut data) = store.data.lock() else {
        return;
    };
    match value {
        Some(text) => {
            data.insert(key, Value::String(text));
        }
        None => {
            data.remove(&key);
        }
    }
    store.flush(&data);
}

/// 启动 GUI。`initial_path` 是文件关联/拖拽带进来的那个文件，前端启动后
/// 用 `initial_file` command 取走并直接打开 —— 双击一个 zip 就该看见它的内容，
/// 而不是看见一个还要再点「打开文件…」的空工作台。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(initial_path: Option<String>) {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(TauriAdapter::from_shared(redline_core::action_registry()))
        .setup(move |app| {
            let dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            app.manage(AnnotationStore::load(dir.join("annotations.json")));
            app.manage(InitialFile(initial_path.clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![action_parity_call, redline_actions, initial_file, read_file_bytes, kv_get, kv_set])
        .run(tauri::generate_context!())
        .expect("Redline 桌面壳启动失败");
}
