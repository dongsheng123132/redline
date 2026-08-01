//! `document.apply-track-changes` —— 唯一一个会写文件的动作。
//!
//! 五条不可协商的安全约束，全部在这里强制，不靠调用方自觉：
//!
//! 1. **只处理 .docx**。只有 Word 有原生 Track Changes 能承载修订痕迹，
//!    pptx/xlsx/pdf 没有对应机制，与其自己发明一套不如不做。
//! 2. **输出必须是新文件**，路径不能等于输入；已存在的输出没有 `--force` 不覆盖。
//! 3. **`expected_input_sha256` 是状态版本号**。原件在出 patch 之后被改过，
//!    这个 patch 就是陈旧的，直接拒绝——不允许「最后写入者获胜」当默认。
//! 4. **替换点必须唯一命中**。命中 0 次或 2 次以上一律拒绝，让人把 patch 写精确，
//!    而不是让核心猜「大概是想改哪个」。
//! 5. **原件永远不动**。回滚 = 删掉输出文件。

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::action_id;
use crate::error::{RedlineError, Result};
use crate::inspect::{resolve, sha256_hex};
use crate::ooxml::Parts;

#[derive(Debug, Clone, Deserialize)]
pub struct Patch {
    pub version: u32,
    #[serde(default)]
    pub action_id: Option<String>,
    /// 出 patch 时原件的 sha256。给了就必须对得上，对不上说明原件已变。
    #[serde(default)]
    pub expected_input_sha256: Option<String>,
    pub changes: Vec<PatchChange>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PatchChange {
    pub op: String,
    pub old: String,
    pub new: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedChange {
    pub index: usize,
    pub op: String,
    pub old: String,
    pub new: String,
    pub reason: Option<String>,
    pub revision_id: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    pub input: FileRef,
    pub output: FileRef,
    pub author: String,
    pub created_at: String,
    pub applied: Vec<AppliedChange>,
    pub rollback: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileRef {
    pub path: String,
    pub sha256: String,
}

pub struct ApplyOptions<'a> {
    pub author: &'a str,
    pub force: bool,
}

pub fn apply_docx(input: &str, patch: &Patch, output: &str, options: ApplyOptions<'_>) -> Result<Audit> {
    let source = resolve(input)?;
    if source.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()) != Some("docx".into()) {
        return Err(RedlineError::input(
            "apply_format_unsupported",
            "apply 当前只支持 .docx —— 只有 Word 有原生 Track Changes 能承载修订痕迹。",
        )
        .with_details(json!({ "input": source.display().to_string() })));
    }

    let target = std::path::PathBuf::from(output);
    let target = if target.is_absolute() { target } else { std::env::current_dir()?.join(target) };

    if same_file(&source, &target) {
        return Err(RedlineError::refused("output_would_overwrite_input", "安全限制：输出路径不能等于输入路径。请保留原件并指定新文件。")
            .with_details(json!({ "path": target.display().to_string() })));
    }
    if target.exists() && !options.force {
        return Err(RedlineError::refused(
            "output_exists",
            format!("输出文件已存在：{}。确认要覆盖这个输出副本请加 --force。", target.display()),
        )
        .with_details(json!({ "output": target.display().to_string() })));
    }

    validate_patch(patch)?;

    let original = std::fs::read(&source)?;
    let actual_hash = sha256_hex(&original);
    if let Some(expected) = &patch.expected_input_sha256 {
        if expected != &actual_hash {
            return Err(RedlineError::refused(
                "stale_patch",
                "原文件在出 patch 之后已被改动，拒绝把陈旧 patch 写进新副本。请重新 inspect 再出 patch。",
            )
            .with_details(json!({
                "expected_input_sha256": expected,
                "actual_input_sha256": actual_hash,
            })));
        }
    }

    let mut parts = Parts::read(&source)?;
    let mut document = parts.require_str("word/document.xml")?;
    let created_at = now_rfc3339();
    let mut revision_id = next_revision_id(&document);
    let mut applied = Vec::with_capacity(patch.changes.len());

    for (index, change) in patch.changes.iter().enumerate() {
        document = replace_single_run(&document, &change.old, &change.new, revision_id, options.author, &created_at)
            .map_err(|err| err.with_details(json!({ "change_index": index, "old": change.old })))?;
        applied.push(AppliedChange {
            index,
            op: change.op.clone(),
            old: change.old.clone(),
            new: change.new.clone(),
            reason: change.reason.clone(),
            revision_id,
        });
        revision_id += 1;
    }

    parts.set("word/document.xml", document.into_bytes());
    ensure_track_revisions(&mut parts)?;
    parts.write(&target)?;

    let output_data = std::fs::read(&target)?;
    Ok(Audit {
        input: FileRef { path: source.display().to_string(), sha256: actual_hash },
        output: FileRef { path: target.display().to_string(), sha256: sha256_hex(&output_data) },
        author: options.author.to_string(),
        created_at,
        applied,
        rollback: "删除输出文件即可回到原件；原输入文件未被修改。",
    })
}

fn validate_patch(patch: &Patch) -> Result<()> {
    if patch.version != 1 {
        return Err(RedlineError::input("bad_patch", "patch 的 version 必须是 1").with_details(json!({ "version": patch.version })));
    }
    if let Some(id) = &patch.action_id {
        if id != action_id::APPLY {
            return Err(RedlineError::input("bad_patch", format!("patch 的 action_id 必须是 {}", action_id::APPLY))
                .with_details(json!({ "action_id": id })));
        }
    }
    if patch.changes.is_empty() {
        return Err(RedlineError::input("bad_patch", "patch 的 changes 不能为空"));
    }
    for (index, change) in patch.changes.iter().enumerate() {
        if change.op != "replace_text" {
            return Err(RedlineError::input("bad_patch", format!("changes[{index}] 只支持 op=replace_text，收到 {}", change.op)));
        }
        if change.old.is_empty() {
            return Err(RedlineError::input("bad_patch", format!("changes[{index}] 的 old 不能为空")));
        }
    }
    Ok(())
}

/// 找出下一个可用的修订 id。Word 要求同一份文档里 `w:id` 不重复。
fn next_revision_id(document_xml: &str) -> u32 {
    let re = regex::Regex::new(r#"w:id="(\d+)""#).expect("常量正则");
    re.captures_iter(document_xml).filter_map(|c| c.get(1)?.as_str().parse::<u32>().ok()).max().unwrap_or(0) + 1
}

/// 把唯一命中的那个 run 换成 `<w:del>旧</w:del><w:ins>新</w:ins>`。
///
/// 不用带 lookahead 的正则（Rust 的 regex 不支持，而且那种写法在嵌套上很脆），
/// 改成手工扫 run 边界 —— OOXML 里 `w:r` 不嵌套，扫描是可靠的。
fn replace_single_run(document_xml: &str, old: &str, new: &str, revision_id: u32, author: &str, date: &str) -> Result<String> {
    let old_xml = xml_escape(old);
    let new_xml = xml_escape(new);

    let hits: Vec<(usize, usize)> =
        find_runs(document_xml).into_iter().filter(|(start, end)| run_text_equals(&document_xml[*start..*end], &old_xml)).collect();

    if hits.len() != 1 {
        return Err(RedlineError::refused(
            "ambiguous_replacement",
            format!(
                "替换项「{}」必须在一个 Word 文本节点中唯一出现；实际匹配 {} 次。请把 patch 写得更精确。",
                truncate(old, 80),
                hits.len()
            ),
        )
        .with_details(json!({ "matches": hits.len() })));
    }

    let (start, end) = hits[0];
    let run = &document_xml[start..end];
    if count_text_nodes(run) != 1 {
        return Err(RedlineError::refused("multi_text_run", "该替换命中一个含多个文本节点的 Word run；为避免破坏格式，核心拒绝自动修改。"));
    }

    let deleted = rewrite_text_node(run, |attrs, _| format!("<w:delText{attrs}>{old_xml}</w:delText>"));
    let inserted = rewrite_text_node(run, |attrs, _| format!("<w:t{attrs}>{new_xml}</w:t>"));
    let meta = format!(r#" w:id="{revision_id}" w:author="{}" w:date="{date}""#, xml_escape(author));
    let revision = format!("<w:del{meta}>{deleted}</w:del><w:ins{meta}>{inserted}</w:ins>");

    let mut out = String::with_capacity(document_xml.len() + revision.len());
    out.push_str(&document_xml[..start]);
    out.push_str(&revision);
    out.push_str(&document_xml[end..]);
    Ok(out)
}

/// 扫出所有 `<w:r ...>...</w:r>` 的字节区间。
///
/// 关键细节：`<w:rPr>`、`<w:rFonts>` 也以 `<w:r` 开头，必须靠「下一个字符是
/// `>` / 空白 / `/`」把它们排除掉。
fn find_runs(xml: &str) -> Vec<(usize, usize)> {
    const OPEN: &str = "<w:r";
    const CLOSE: &str = "</w:r>";
    let bytes = xml.as_bytes();
    let mut runs = Vec::new();
    let mut cursor = 0usize;

    while let Some(offset) = xml[cursor..].find(OPEN) {
        let start = cursor + offset;
        let after = start + OPEN.len();
        let next = bytes.get(after).copied();
        let is_run_tag = matches!(next, Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n') | Some(b'/'));
        if !is_run_tag {
            cursor = after;
            continue;
        }
        // 自闭合的空 run（<w:r/>）不含文本，跳过
        match xml[start..].find(CLOSE) {
            Some(close_offset) => {
                let end = start + close_offset + CLOSE.len();
                runs.push((start, end));
                cursor = end;
            }
            None => break,
        }
    }
    runs
}

/// run 里 `<w:t>` 的文本是否恰好等于目标（已转义的）文本。
fn run_text_equals(run: &str, escaped_old: &str) -> bool {
    text_node_content(run).map(|text| text == escaped_old).unwrap_or(false)
}

fn text_node_content(run: &str) -> Option<&str> {
    let open = run.find("<w:t")?;
    let after = open + "<w:t".len();
    // 排除 <w:tab/>、<w:tbl> 之类以 <w:t 开头的别的标签
    if !matches!(run.as_bytes().get(after), Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n')) {
        return None;
    }
    let content_start = open + run[open..].find('>')? + 1;
    let content_end = content_start + run[content_start..].find("</w:t>")?;
    Some(&run[content_start..content_end])
}

fn count_text_nodes(run: &str) -> usize {
    let mut count = 0usize;
    let mut cursor = 0usize;
    while let Some(offset) = run[cursor..].find("<w:t") {
        let open = cursor + offset;
        let after = open + "<w:t".len();
        if matches!(run.as_bytes().get(after), Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n')) {
            count += 1;
        }
        cursor = after;
    }
    count
}

/// 把 run 里唯一那个 `<w:t ATTRS>内容</w:t>` 交给 `build` 重写，其余原样保留。
fn rewrite_text_node(run: &str, build: impl Fn(&str, &str) -> String) -> String {
    let Some(open) = run.find("<w:t") else {
        return run.to_string();
    };
    let Some(gt_offset) = run[open..].find('>') else {
        return run.to_string();
    };
    let tag_end = open + gt_offset;
    let attrs = &run[open + "<w:t".len()..tag_end];
    let content_start = tag_end + 1;
    let Some(close_offset) = run[content_start..].find("</w:t>") else {
        return run.to_string();
    };
    let content_end = content_start + close_offset;
    let content = &run[content_start..content_end];

    let mut out = String::with_capacity(run.len());
    out.push_str(&run[..open]);
    out.push_str(&build(attrs, content));
    out.push_str(&run[content_end + "</w:t>".len()..]);
    out
}

/// 确保文档打开时处于「修订模式」，否则 Word 里看不到修订痕迹的提示条。
fn ensure_track_revisions(parts: &mut Parts) -> Result<()> {
    const NAME: &str = "word/settings.xml";
    let Some(raw) = parts.get(NAME) else {
        return Ok(());
    };
    let settings = String::from_utf8_lossy(raw).to_string();
    if settings.contains("<w:trackRevisions") {
        return Ok(());
    }
    let patched = settings.replace("</w:settings>", "<w:trackRevisions/></w:settings>");
    parts.set(NAME, patched.into_bytes());
    Ok(())
}

fn xml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// RFC3339 UTC 时间戳（秒精度），例如 `2026-07-31T10:20:30Z`。
///
/// 自己算而不是引入日期库：只需要这一个格式，多一个依赖不值。
fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant 的 civil_from_days —— 把 Unix 天数转成年月日。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 把审计信息转成信封 payload。
pub fn audit_json(audit: &Audit) -> Value {
    serde_json::to_value(audit).unwrap_or_else(|_| json!({}))
}

#[cfg(test)]
#[allow(non_snake_case)] // 测试名是中文夹 XML/rPr 这类专有名词，大小写按原样保留才读得懂
mod tests {
    use super::*;

    const DOC: &str = r#"<w:document xmlns:w="x"><w:body><w:p><w:pPr><w:rPr><w:b/></w:rPr></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">旧文案</w:t></w:r><w:r><w:t>别的</w:t></w:r></w:p></w:body></w:document>"#;

    #[test]
    fn 唯一命中时生成删除加插入的修订对() {
        let out = replace_single_run(DOC, "旧文案", "新文案", 7, "Codex", "2026-07-31T00:00:00Z").unwrap();
        assert!(out.contains(r#"<w:del w:id="7" w:author="Codex" w:date="2026-07-31T00:00:00Z">"#));
        assert!(out.contains("<w:delText xml:space=\"preserve\">旧文案</w:delText>"));
        assert!(out.contains("<w:t xml:space=\"preserve\">新文案</w:t>"));
        // 原来的格式属性（加粗）必须留着
        assert!(out.contains("<w:rPr><w:b/></w:rPr>"));
        // 没被改的 run 原样保留
        assert!(out.contains("<w:r><w:t>别的</w:t></w:r>"));
    }

    #[test]
    fn 命中零次或多次一律拒绝() {
        let err = replace_single_run(DOC, "不存在", "x", 1, "a", "d").unwrap_err();
        assert_eq!(err.code, "ambiguous_replacement");

        let twice = DOC.replace("<w:r><w:t>别的</w:t></w:r>", "<w:r><w:t>旧文案</w:t></w:r>");
        let err = replace_single_run(&twice, "旧文案", "x", 1, "a", "d").unwrap_err();
        assert_eq!(err.code, "ambiguous_replacement");
    }

    #[test]
    fn rPr_不会被误认成_run() {
        // <w:rPr> 以 <w:r 开头，如果边界判断写错，run 区间会从 rPr 开始导致替换错位
        let runs = find_runs(DOC);
        assert_eq!(runs.len(), 2);
        assert!(DOC[runs[0].0..].starts_with("<w:r>"));
    }

    #[test]
    fn 特殊字符按_XML_转义后再匹配() {
        let doc = r#"<w:p xmlns:w="x"><w:r><w:t>A &amp; B &lt;C&gt;</w:t></w:r></w:p>"#;
        let out = replace_single_run(doc, "A & B <C>", "D", 1, "a", "d").unwrap();
        assert!(out.contains("<w:delText>A &amp; B &lt;C&gt;</w:delText>"));
        assert!(out.contains("<w:t>D</w:t>"));
    }

    #[test]
    fn 修订编号避开已用过的() {
        assert_eq!(next_revision_id(r#"<w:ins w:id="3"/><w:del w:id="11"/>"#), 12);
        assert_eq!(next_revision_id("<w:p/>"), 1);
    }

    #[test]
    fn 时间戳格式正确() {
        // 2026-07-31T00:00:00Z 对应的 Unix 天数
        assert_eq!(civil_from_days(20_665), (2026, 7, 31));
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        let now = now_rfc3339();
        assert_eq!(now.len(), 20, "{now}");
        assert!(now.ends_with('Z'));
    }

    #[test]
    fn 空_patch_与错误_op_被拒绝() {
        let empty = Patch { version: 1, action_id: None, expected_input_sha256: None, changes: vec![] };
        assert_eq!(validate_patch(&empty).unwrap_err().code, "bad_patch");

        let bad_op = Patch {
            version: 1,
            action_id: None,
            expected_input_sha256: None,
            changes: vec![PatchChange { op: "delete".into(), old: "a".into(), new: "".into(), reason: None }],
        };
        assert_eq!(validate_patch(&bad_op).unwrap_err().code, "bad_patch");
    }
}
