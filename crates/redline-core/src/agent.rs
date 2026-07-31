//! `agent.dispatch` —— 把「人的标注 + 对应内容 + 源文件」交给某个 AI CLI 去改。
//!
//! 这是 Redline 三栏界面里**中间那栏**的实质：左栏是原文和标注，中栏选用哪个
//! agent 并派发，右栏预览 agent 产出的新文件。
//!
//! 三条纪律：
//!
//! 1. **Redline 自己永远不写源文件**，agent 也不许写。派给 agent 的指令里写死了
//!    「读 SOURCE、写 OUTPUT、绝不改 SOURCE」，而且 `OUTPUT` 由 Redline 指定。
//! 2. **跑什么命令必须能被看见**。返回的信封里带完整 `command` 和 `prompt`，
//!    包括那些放开写权限的参数 —— 不藏在代码里替用户做决定。
//! 3. **必须有超时**。外部 agent 会卡（等网络、等模型、等一个它自己在等的东西），
//!    没有超时的子进程会把界面挂死。

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use crate::error::{RedlineError, Result};
use crate::inspect;
use crate::paths;

/// 默认超时 10 分钟。改文档的活儿再慢也该在这个量级内出结果。
const DEFAULT_TIMEOUT_SECS: u64 = 600;
/// 回传的 stdout/stderr 上限。agent 的日志可能非常长，全塞进信封没意义。
const OUTPUT_TAIL_BYTES: usize = 8 * 1024;

/// 一个 AI CLI 的调用规约。
///
/// 新增 agent 只加一条，不改派发逻辑。
///
/// **提示词一律走 stdin，不走命令行参数。** 两个原因：
/// 1. Windows 上 `codex` / `claude` 都是 npm 装的 `.cmd` shim，参数要过 cmd.exe
///    再解析一遍；提示词里带换行、`&`、`|`、引号都会被 cmd 吃掉或改写。
/// 2. Windows 命令行总长约 32KB，长文档的工单能撑爆。
pub struct AgentSpec {
    pub id: &'static str,
    pub label: &'static str,
    /// 可执行文件名（走 PATH 查找，Windows 上会自动带上 .cmd/.exe 等后缀）。
    pub program: &'static str,
    /// 固定参数。**放开写权限的参数就写在这里，明面上的，会原样出现在返回的 command 里。**
    pub args: &'static [&'static str],
    /// 这个 agent 的写权限是怎么给的 —— 直接说人话，用户要能读懂自己授了什么。
    pub permission_note: &'static str,
}

pub const AGENTS: &[AgentSpec] = &[
    AgentSpec {
        id: "claude",
        label: "Claude Code",
        // -p 且不带 prompt 参数时从 stdin 读
        program: "claude",
        args: &["-p", "--permission-mode", "acceptEdits"],
        permission_note: "acceptEdits：允许在工作目录内改文件，不允许跑任意命令。",
    },
    AgentSpec {
        id: "codex",
        label: "Codex CLI",
        // 末尾的 - 表示从 stdin 读提示词。
        // --skip-git-repo-check：codex 默认只肯在 git 仓库里跑（怕改坏没版本控制的东西）。
        // 但文档不可能都放在 git 仓里，这条不放开 Redline 就没法用在普通文件夹上。
        // 沙箱本身没放松，仍是 workspace-write。
        program: "codex",
        args: &["exec", "--sandbox", "workspace-write", "--skip-git-repo-check", "-"],
        permission_note: "workspace-write：允许在工作目录内改文件，网络与目录外写入受限；\
                          并跳过 codex 的 git 仓库检查（文档目录通常不是 git 仓）。",
    },
];

pub fn lookup(id: &str) -> Option<&'static AgentSpec> {
    AGENTS.iter().find(|spec| spec.id == id)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    /// 锚在哪个 unit 上（`paragraph:3` / `slide:2` / `sheet:1`）。
    /// 没有 unit（比如纯图片上画的框）时为 None，那就只能靠 note 描述。
    pub unit_id: Option<String>,
    /// 人写的批注。
    pub note: String,
    /// 该 unit 当前的正文，由核心从快照里补上 —— 让 agent 既看得见位置也读得懂内容。
    pub unit_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchReport {
    pub agent: String,
    pub label: String,
    /// 实际执行的完整命令行，逐段列出。用户要能一眼看清授了什么权限。
    pub command: Vec<String>,
    pub permission_note: String,
    pub cwd: String,
    pub prompt: String,
    pub source: String,
    pub output: String,
    /// dry_run 时下面这些是 None —— 什么都没跑。
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: Option<u128>,
    pub stdout_tail: Option<String>,
    pub stderr_tail: Option<String>,
    /// agent 跑完之后 output 到底存不存在。**这才是「成没成」的判据**，
    /// 不是退出码 —— agent 可以退出码 0 但什么也没写。
    pub output_exists: bool,
}

pub struct DispatchRequest<'a> {
    pub agent: &'a str,
    pub source: &'a str,
    /// 生成标注/影文档时看到的原件哈希。给了就必须与派发瞬间的源文件一致。
    pub expected_source_sha256: Option<String>,
    pub output: &'a str,
    pub annotations: Vec<Annotation>,
    pub instruction: Option<String>,
    pub cwd: Option<String>,
    pub timeout_secs: u64,
    pub dry_run: bool,
}

pub fn dispatch(request: DispatchRequest<'_>) -> Result<DispatchReport> {
    let spec = lookup(request.agent).ok_or_else(|| {
        RedlineError::input("unknown_agent", format!("不认识的 agent：{}", request.agent))
            .with_details(json!({ "known": AGENTS.iter().map(|a| a.id).collect::<Vec<_>>() }))
    })?;

    let source = inspect::resolve(request.source)?;
    let source_display = paths::display(&source);

    if let Some(expected) = request.expected_source_sha256.as_deref() {
        let actual = inspect::sha256_hex(&std::fs::read(&source)?);
        if actual != expected {
            return Err(RedlineError::refused(
                "stale_shadow",
                "源文件在预览/标注之后已经改变；旧影文档和标注不能继续派发。请重新打开文件确认。",
            )
            .with_details(json!({
                "path": source_display,
                "expectedSha256": expected,
                "actualSha256": actual,
            })));
        }
    }

    let output = std::path::PathBuf::from(request.output);
    let output = if output.is_absolute() { output } else { std::env::current_dir()?.join(output) };
    let output = paths::normalize(output);
    let output_display = paths::display(&output);

    if source == output {
        return Err(RedlineError::refused(
            "output_would_overwrite_input",
            "派发目标不能是源文件本身。Redline 的规矩是 agent 写新文件，源文件永远不动。",
        )
        .with_details(json!({ "path": output_display })));
    }

    // 标注里缺正文的，从快照补上 —— agent 拿到的必须是「位置 + 内容」，光有坐标没用
    let annotations = enrich(&source_display, request.annotations)?;

    let cwd = match &request.cwd {
        Some(dir) => std::path::PathBuf::from(dir),
        None => source.parent().map(|p| p.to_path_buf()).unwrap_or(std::env::current_dir()?),
    };
    let cwd_display = paths::display(&cwd);

    let prompt = build_prompt(&source_display, &output_display, &annotations, request.instruction.as_deref());

    // 提示词走 stdin，命令行里如实标出来，别让人以为提示词藏在某个参数里
    let mut command: Vec<String> = vec![spec.program.to_string()];
    command.extend(spec.args.iter().map(|a| a.to_string()));
    command.push("<提示词经 stdin 传入>".to_string());

    let mut report = DispatchReport {
        agent: spec.id.to_string(),
        label: spec.label.to_string(),
        command,
        permission_note: spec.permission_note.to_string(),
        cwd: cwd_display,
        prompt,
        source: source_display,
        output: output_display.clone(),
        exit_code: None,
        timed_out: false,
        duration_ms: None,
        stdout_tail: None,
        stderr_tail: None,
        output_exists: false,
    };

    if request.dry_run {
        return Ok(report);
    }

    let started = Instant::now();
    let outcome = run(spec, &report.prompt, &cwd, request.timeout_secs)?;
    report.duration_ms = Some(started.elapsed().as_millis());
    report.exit_code = outcome.exit_code;
    report.timed_out = outcome.timed_out;
    report.stdout_tail = Some(outcome.stdout);
    report.stderr_tail = Some(outcome.stderr);
    report.output_exists = std::path::Path::new(&output_display).is_file();

    Ok(report)
}

fn enrich(source: &str, annotations: Vec<Annotation>) -> Result<Vec<Annotation>> {
    let needs_text = annotations.iter().any(|a| a.unit_text.is_none() && a.unit_id.is_some());
    if !needs_text {
        return Ok(annotations);
    }
    // 拿不到快照不算致命 —— 图片这类格式本来就没有 unit，标注照样派得出去
    let snapshot = match inspect::inspect(source) {
        Ok(snapshot) => snapshot,
        Err(_) => return Ok(annotations),
    };
    Ok(annotations
        .into_iter()
        .map(|mut annotation| {
            if annotation.unit_text.is_none() {
                if let Some(id) = &annotation.unit_id {
                    annotation.unit_text = snapshot.units.iter().find(|u| &u.id == id).map(|u| u.text.clone());
                }
            }
            annotation
        })
        .collect())
}

/// 拼给 agent 的提示词。
///
/// 刻意写得像一张工单而不是一句闲聊：agent 拿到的是明确的输入路径、输出路径、
/// 逐条标注、以及「不许动源文件」这条硬约束。
fn build_prompt(source: &str, output: &str, annotations: &[Annotation], instruction: Option<&str>) -> String {
    let mut prompt = String::new();
    prompt.push_str("你在给 Redline 当文档修改后端。请严格按下面的工单执行。\n\n");
    prompt.push_str(&format!("源文件（只读，绝对不要修改它）：{source}\n"));
    prompt.push_str(&format!("输出文件（把改好的完整文档写到这里，不存在就创建）：{output}\n\n"));

    if let Some(text) = instruction {
        if !text.trim().is_empty() {
            prompt.push_str(&format!("整体要求：{}\n\n", text.trim()));
        }
    }

    if annotations.is_empty() {
        prompt.push_str("本次没有逐条标注，按上面的整体要求处理全文。\n");
    } else {
        prompt.push_str(&format!("人在文档上留了 {} 条标注，逐条处理：\n\n", annotations.len()));
        for (index, annotation) in annotations.iter().enumerate() {
            prompt.push_str(&format!("【标注 {}】", index + 1));
            match &annotation.unit_id {
                Some(id) => prompt.push_str(&format!(" 位置 {id}\n")),
                None => prompt.push('\n'),
            }
            if let Some(text) = &annotation.unit_text {
                prompt.push_str(&format!("  原文：{}\n", text.trim()));
            }
            prompt.push_str(&format!("  要求：{}\n\n", annotation.note.trim()));
        }
    }

    prompt.push_str(
        "硬约束：\n\
         1. 源文件保持字节不变。任何情况下都不要覆盖、移动或删除它。\n\
         2. 结果写进上面那个输出路径，格式与源文件一致。\n\
         3. 只改标注点到的地方，其余内容原样保留。\n\
         4. 做完在最后一行输出 DONE 加输出文件路径。\n",
    );
    prompt
}

struct Outcome {
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: String,
    stderr: String,
}

fn run(spec: &AgentSpec, prompt: &str, cwd: &std::path::Path, timeout_secs: u64) -> Result<Outcome> {
    let timeout = Duration::from_secs(if timeout_secs == 0 { DEFAULT_TIMEOUT_SECS } else { timeout_secs });

    // 必须用解析后的完整路径。Windows 上 npm 装的 CLI 是 `codex.cmd` 这样的批处理
    // shim，`Command::new("codex")` 只会去找 codex.exe，找不到就报「没装」——
    // 明明装了却说没装，是最误导人的那种错误。
    let program = resolve_program(spec.program).ok_or_else(|| {
        RedlineError::input(
            "agent_not_installed",
            format!("找不到 {}（命令 `{}`）。装好并确保它在 PATH 里再试。", spec.label, spec.program),
        )
        .with_details(json!({ "agent": spec.id, "program": spec.program }))
    })?;

    let mut child = Command::new(&program)
        .args(spec.args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| RedlineError::internal("agent_spawn_failed", format!("{}：{err}", paths::display(&program))))?;

    // 提示词写进 stdin 再关掉 —— 不关的话 agent 会一直等更多输入
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(prompt.as_bytes());
    }

    // 子进程的 stdout/stderr 各开一个线程读干净，否则管道写满会让它卡死
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let stdout_thread = std::thread::spawn(move || drain(&mut stdout_pipe));
    let stderr_thread = std::thread::spawn(move || drain(&mut stderr_pipe));

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    // 超时就杀掉。留着一个卡死的子进程比报错更糟。
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(err) => return Err(RedlineError::internal("agent_wait_failed", err.to_string())),
        }
    };

    let stdout = stdout_thread.join().unwrap_or_default();
    let stderr = stderr_thread.join().unwrap_or_default();

    Ok(Outcome { exit_code: status.and_then(|s| s.code()), timed_out, stdout: tail(&stdout), stderr: tail(&stderr) })
}

fn drain(pipe: &mut Option<impl Read>) -> String {
    let Some(reader) = pipe else {
        return String::new();
    };
    let mut buffer = Vec::new();
    let _ = reader.read_to_end(&mut buffer);
    String::from_utf8_lossy(&buffer).to_string()
}

/// 只留尾部 —— agent 的关键结论一般在最后，开头多是启动噪音。
fn tail(text: &str) -> String {
    if text.len() <= OUTPUT_TAIL_BYTES {
        return text.to_string();
    }
    let start = text.len() - OUTPUT_TAIL_BYTES;
    // 别从多字节字符中间切开
    let start = (start..text.len()).find(|i| text.is_char_boundary(*i)).unwrap_or(text.len());
    format!("…（已截断前 {} 字节）\n{}", start, &text[start..])
}

/// agent 清单，给界面画选择器用。
pub fn catalog() -> Value {
    json!({
        "agents": AGENTS
            .iter()
            .map(|spec| json!({
                "id": spec.id,
                "label": spec.label,
                "program": spec.program,
                "args": spec.args,
                "permissionNote": spec.permission_note,
                "installed": resolve_program(spec.program).is_some(),
                "resolvedPath": resolve_program(spec.program).map(|p| paths::display(&p)),
            }))
            .collect::<Vec<_>>(),
    })
}

/// 在 PATH 里把命令名解析成**完整可执行路径**。
///
/// 返回完整路径而不是布尔值，是因为 Windows 上必须拿着完整路径去 spawn：
/// npm 装的 CLI 是 `codex.cmd` 这类批处理 shim，只给命令名的话
/// `Command` 只会去找 `codex.exe`。清单里报「已安装」而派发时报「没装」，
/// 就是这么来的。
fn resolve_program(program: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    // Windows 按 PATHEXT 的顺序试后缀；.cmd 是 npm shim 最常见的形态
    let extensions: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| e.to_ascii_lowercase())
            .collect()
    } else {
        Vec::new()
    };

    for dir in std::env::split_paths(&path) {
        // 非 Windows（以及本身就带后缀的情况）直接命中
        let bare = dir.join(program);
        if bare.is_file() && (!cfg!(windows) || bare.extension().is_some()) {
            return Some(bare);
        }
        for ext in &extensions {
            let candidate = dir.join(format!("{program}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        // Windows 上无后缀的同名文件（Git Bash 的 sh 脚本）当兜底，排在最后
        if bare.is_file() {
            return Some(bare);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn annotation(unit: &str, note: &str, text: &str) -> Annotation {
        Annotation { unit_id: Some(unit.into()), note: note.into(), unit_text: Some(text.into()) }
    }

    #[test]
    fn 提示词带齐输入输出和不许改源文件的硬约束() {
        let prompt = build_prompt(
            r"C:\a\原件.docx",
            r"C:\a\改后.docx",
            &[annotation("paragraph:2", "改得更正式", "项目申报表")],
            Some("整体语气正式一点"),
        );
        assert!(prompt.contains(r"C:\a\原件.docx"));
        assert!(prompt.contains(r"C:\a\改后.docx"));
        assert!(prompt.contains("绝对不要修改"));
        assert!(prompt.contains("位置 paragraph:2"));
        assert!(prompt.contains("原文：项目申报表"));
        assert!(prompt.contains("改得更正式"));
        assert!(prompt.contains("整体语气正式一点"));
    }

    #[test]
    fn 没有标注时也给得出可执行的工单() {
        let prompt = build_prompt("a.docx", "b.docx", &[], Some("全文润色"));
        assert!(prompt.contains("本次没有逐条标注"));
        assert!(prompt.contains("全文润色"));
    }

    #[test]
    fn 不认识的_agent_被拒绝() {
        let err = dispatch(DispatchRequest {
            agent: "gemini",
            source: "a.docx",
            expected_source_sha256: None,
            output: "b.docx",
            annotations: vec![],
            instruction: None,
            cwd: None,
            timeout_secs: 1,
            dry_run: true,
        })
        .unwrap_err();
        assert_eq!(err.code, "unknown_agent");
    }

    #[test]
    fn 源文件变化后拒绝旧影文档派发() {
        let unique = format!(
            "redline-stale-shadow-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        );
        let source = std::env::temp_dir().join(format!("{unique}.txt"));
        let output = std::env::temp_dir().join(format!("{unique}-out.txt"));
        std::fs::write(&source, "新内容").unwrap();

        let source_text = source.to_string_lossy().to_string();
        let output_text = output.to_string_lossy().to_string();
        let err = dispatch(DispatchRequest {
            agent: "codex",
            source: &source_text,
            expected_source_sha256: Some("旧哈希".into()),
            output: &output_text,
            annotations: vec![],
            instruction: None,
            cwd: None,
            timeout_secs: 1,
            dry_run: true,
        })
        .unwrap_err();

        let _ = std::fs::remove_file(&source);
        assert_eq!(err.code, "stale_shadow");
        assert_eq!(err.class, crate::error::ErrorClass::Refused);
    }

    #[test]
    fn 截断保留尾部且不切碎多字节字符() {
        let long = "中".repeat(OUTPUT_TAIL_BYTES);
        let cut = tail(&long);
        assert!(cut.starts_with("…（已截断"));
        // 能正常当字符串用说明没从字符中间切
        assert!(cut.chars().count() > 0);
    }

    #[test]
    fn agent_清单带安装状态() {
        let catalog = catalog();
        let agents = catalog["agents"].as_array().unwrap();
        assert_eq!(agents.len(), 2);
        assert!(agents.iter().all(|a| a["installed"].is_boolean()));
        // 权限说明必须有，用户要能读懂自己授了什么
        assert!(agents.iter().all(|a| !a["permissionNote"].as_str().unwrap_or("").is_empty()));
    }
}
