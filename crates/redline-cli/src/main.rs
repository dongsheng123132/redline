//! Redline CLI —— 动作核心的**调用方**，不是第二份实现。
//!
//! 这个文件里不该出现任何「怎么解析 docx」「什么算不安全路径」之类的判断。
//! 每个子命令做且只做三件事：把 argv 拼成 params、调 [`redline_core::dispatch`]、
//! 把信封打印出来。GUI 和 MCP 走的是同一个 dispatch，所以三个界面行为天然一致。
//!
//! ## 给机器用的约定
//!
//! - **stdout 只出结果**（JSON 或人类摘要），日志和错误详情走 stderr。
//! - **非 TTY 自动切 JSON**，管道里拿到的永远是可解析的东西，不带 ANSI。
//! - **`ok` 是稳定成功位**，成功失败同一个信封形状，判一个布尔就够。
//! - **退出码**：0 成功 / 1 输入错误 / 2 安全拒绝 / 3 内部错误 / 64 用法错误。
//!   「安全拒绝」单独占一个码，是因为它不是 bug —— 脚本应当把它当成需要人介入，
//!   而不是重试。

use std::io::{IsTerminal, Write};

use clap::{Parser, Subcommand};
use redline_core::{action_id, dispatch};
use serde_json::{json, Value};

/// 用法错误（参数拼不出合法请求）。沿用 sysexits.h 的 EX_USAGE。
const EXIT_USAGE: i32 = 64;

#[derive(Parser)]
#[command(
    name = "redline",
    version,
    about = "Redline —— 面向 AI 时代的万能文档预览标记层（CLI 面）",
    long_about = "Redline CLI —— 动作核心的命令行入口。\n\n\
退出码：\n  \
0   成功\n  \
1   输入错误（文件不存在、格式不支持、patch 非法）\n  \
2   安全拒绝（会覆盖原件、原件已变更、替换点不唯一、压缩包路径越界）\n  \
3   内部错误\n  \
64  用法错误\n\n\
stdout 只出结果，日志走 stderr；管道里自动输出 JSON。"
)]
struct Cli {
    /// 强制 JSON 输出（管道里默认就是 JSON，这个开关是给 TTY 下用的）
    #[arg(long, global = true)]
    json: bool,

    /// 结果同时写一份到文件（stdout 照常输出）
    #[arg(long, global = true, value_name = "FILE")]
    out: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 解析文件成结构化快照（只读）
    Inspect { file: String },

    /// 对比两个同格式文件（只读）
    Diff { before: String, after: String },

    /// 检查文件能否被解析（只读）
    Verify { file: String },

    /// 列出全部支持格式及其能力（只读）
    Formats,

    /// 列出全部业务动作 —— 这就是绑定清单，可被机器检查
    Actions,

    /// 压缩包动作
    #[command(subcommand)]
    Archive(ArchiveCommand),

    /// 把 patch 写成带 Word 修订痕迹的**新文件**（原件永远不动）
    Apply {
        input: String,
        /// patch JSON 文件路径
        patch: String,
        /// 输出路径，必须不同于 input
        output: String,
        #[arg(long, default_value = "AI Redline")]
        author: String,
        /// 审计记录另存一份
        #[arg(long, value_name = "FILE")]
        audit: Option<String>,
        /// 允许覆盖已存在的**输出副本**（永远不能覆盖 input）
        #[arg(long)]
        force: bool,
    },

    /// 通用逃生口：按 action id 直接调核心，跟 MCP 走同一入口
    Call {
        /// 动作 id，例如 document.inspect
        action: String,
        /// 参数 JSON，例如 '{"path":"a.docx"}'
        #[arg(long, default_value = "{}")]
        params: String,
    },
}

#[derive(Subcommand)]
enum ArchiveCommand {
    /// 列出压缩包条目，不解包（只读）
    List { file: String },
    /// 解包到目标目录（只写目标目录，原包不动）
    Extract {
        file: String,
        dest: String,
        /// 允许覆盖目标目录里的同名文件
        #[arg(long)]
        overwrite: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let (action, params, audit_out) = match build_request(&cli.command) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(EXIT_USAGE);
        }
    };

    // 本地动作（不进核心的元信息查询）单独处理：绑定清单本身不是业务动作。
    let envelope = match action {
        LOCAL_ACTIONS => local_actions(),
        _ => dispatch(action, &params),
    };

    let ok = envelope["ok"].as_bool().unwrap_or(false);
    emit(&cli, &envelope, audit_out);
    std::process::exit(if ok { 0 } else { exit_code_of(&envelope) });
}

/// `redline actions` 的伪 action id —— 不进核心，因为它问的是「有哪些动作」，
/// 属于元信息，不是业务动作。
const LOCAL_ACTIONS: &str = "__local.actions";

fn build_request(command: &Command) -> Result<(&'static str, Value, Option<String>), String> {
    Ok(match command {
        Command::Inspect { file } => (action_id::INSPECT, json!({ "path": file }), None),
        Command::Verify { file } => (action_id::VERIFY, json!({ "path": file }), None),
        Command::Formats => (action_id::FORMATS, json!({}), None),
        Command::Actions => (LOCAL_ACTIONS, json!({}), None),
        Command::Diff { before, after } => {
            (action_id::DIFF, json!({ "before": before, "after": after }), None)
        }
        Command::Archive(ArchiveCommand::List { file }) => {
            (action_id::ARCHIVE_LIST, json!({ "path": file }), None)
        }
        Command::Archive(ArchiveCommand::Extract { file, dest, overwrite }) => (
            action_id::ARCHIVE_EXTRACT,
            json!({ "path": file, "dest": dest, "overwrite": overwrite }),
            None,
        ),
        Command::Apply { input, patch, output, author, audit, force } => {
            let raw = std::fs::read_to_string(patch)
                .map_err(|e| format!("读不到 patch 文件 {patch}：{e}"))?;
            let parsed: Value = serde_json::from_str(&raw)
                .map_err(|e| format!("patch 不是合法 JSON：{e}"))?;
            (
                action_id::APPLY,
                json!({
                    "input": input,
                    "patch": parsed,
                    "output": output,
                    "author": author,
                    "force": force,
                }),
                audit.clone(),
            )
        }
        Command::Call { action, params } => {
            let parsed: Value =
                serde_json::from_str(params).map_err(|e| format!("--params 不是合法 JSON：{e}"))?;
            // action id 由核心校验，未知的会拿到 unknown_action 错误信封（退出码 1）
            (Box::leak(action.clone().into_boxed_str()), parsed, None)
        }
    })
}

fn local_actions() -> Value {
    json!({
        "ok": true,
        "version": 1,
        "action_id": LOCAL_ACTIONS,
        "actions": redline_core::ACTIONS
            .iter()
            .map(|(id, description)| json!({ "id": id, "description": description }))
            .collect::<Vec<_>>(),
    })
}

fn exit_code_of(envelope: &Value) -> i32 {
    match envelope["error"]["class"].as_str() {
        Some("input") => 1,
        Some("refused") => 2,
        _ => 3,
    }
}

fn emit(cli: &Cli, envelope: &Value, audit_out: Option<String>) {
    let pretty = serde_json::to_string_pretty(envelope).unwrap_or_else(|_| "{}".into());

    // --out 存的永远是 JSON，不管终端那边显示成什么样
    if let Some(path) = &cli.out {
        write_file(path, &pretty);
    }
    if let Some(path) = audit_out {
        write_file(&path, &pretty);
    }

    let machine_readable = cli.json || !std::io::stdout().is_terminal();
    if machine_readable {
        println!("{pretty}");
        return;
    }
    print_human(envelope);
}

fn write_file(path: &str, content: &str) {
    let target = std::path::Path::new(path);
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    if let Err(err) = std::fs::write(target, format!("{content}\n")) {
        // 写不出去只影响副本，不该让主结果丢失 —— 报到 stderr，退出码不变
        let _ = writeln!(std::io::stderr(), "写入 {path} 失败：{err}");
    }
}

/// TTY 下的人类可读摘要。**只是同一份结果的另一种排版**，不是另一套判断。
fn print_human(envelope: &Value) {
    if envelope["ok"].as_bool() != Some(true) {
        let error = &envelope["error"];
        let class = error["class"].as_str().unwrap_or("internal");
        let label = match class {
            "refused" => "已拒绝",
            "input" => "输入错误",
            _ => "内部错误",
        };
        println!("{label}（{}）：{}", error["code"].as_str().unwrap_or("?"), error["message"].as_str().unwrap_or(""));
        if let Some(details) = error.get("details") {
            let _ = writeln!(std::io::stderr(), "详情：{details}");
        }
        return;
    }

    match envelope["action_id"].as_str().unwrap_or_default() {
        action_id::FORMATS => {
            println!("{:<18} {:<24} {}", "格式", "扩展名", "能力");
            for spec in envelope["formats"].as_array().unwrap_or(&vec![]) {
                let extensions = spec["extensions"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "))
                    .unwrap_or_default();
                println!(
                    "{:<18} {:<24} {}",
                    spec["id"].as_str().unwrap_or("?"),
                    truncate(&extensions, 24),
                    capability_summary(&spec["caps"]),
                );
            }
        }
        LOCAL_ACTIONS => {
            for item in envelope["actions"].as_array().unwrap_or(&vec![]) {
                println!("{:<32} {}", item["id"].as_str().unwrap_or("?"), item["description"].as_str().unwrap_or(""));
            }
        }
        action_id::INSPECT | action_id::VERIFY => {
            println!("格式：{}", envelope["format"].as_str().unwrap_or("?"));
            println!("摘要：{}", envelope["summary"]);
            if let Some(units) = envelope["units"].as_array() {
                println!("单元：{} 个", units.len());
                for unit in units.iter().take(5) {
                    println!("  {} {}", unit["label"].as_str().unwrap_or("?"), truncate(unit["text"].as_str().unwrap_or(""), 60));
                }
                if units.len() > 5 {
                    println!("  …还有 {} 个（用 --json 看全部）", units.len() - 5);
                }
            }
        }
        action_id::DIFF => {
            println!("变化：{} 处", envelope["summary"]["changedUnits"]);
            for change in envelope["changes"].as_array().unwrap_or(&vec![]) {
                println!(
                    "  [{}] {}\n    - {}\n    + {}",
                    change["kind"].as_str().unwrap_or("?"),
                    change["label"].as_str().unwrap_or("?"),
                    truncate(change["before"].as_str().unwrap_or(""), 70),
                    truncate(change["after"].as_str().unwrap_or(""), 70),
                );
            }
        }
        action_id::ARCHIVE_LIST => {
            println!("共 {} 条", envelope["count"]);
            for entry in envelope["entries"].as_array().unwrap_or(&vec![]).iter().take(50) {
                println!("  {:<10} {:>10}  {}", entry["format"].as_str().unwrap_or("?"), entry["bytes"], entry["path"].as_str().unwrap_or("?"));
            }
        }
        action_id::ARCHIVE_EXTRACT => {
            println!("解出 {} 个文件", envelope["written"]);
        }
        action_id::APPLY => {
            println!("已写出：{}", envelope["output"]["path"].as_str().unwrap_or("?"));
            println!("修订 {} 处，作者 {}", envelope["applied"].as_array().map(Vec::len).unwrap_or(0), envelope["author"].as_str().unwrap_or("?"));
            println!("回滚：{}", envelope["rollback"].as_str().unwrap_or(""));
        }
        _ => println!("{}", serde_json::to_string_pretty(envelope).unwrap_or_default()),
    }
}

fn capability_summary(caps: &Value) -> String {
    let mut bits = Vec::new();
    for (key, label) in [("inspect", "解析"), ("diff", "对比"), ("apply", "改写"), ("annotate", "标注")] {
        if caps[key].as_bool() == Some(true) {
            bits.push(label);
        }
    }
    match caps["extract"].as_str() {
        Some("native") => bits.push("解包"),
        Some("external_7z") => bits.push("解包(需7z)"),
        _ => {}
    }
    bits.join(" ")
}

fn truncate(value: &str, max_chars: usize) -> String {
    let flat = value.replace('\n', " ");
    if flat.chars().count() <= max_chars {
        return flat;
    }
    flat.chars().take(max_chars.saturating_sub(1)).collect::<String>() + "…"
}
