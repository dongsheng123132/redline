//! 可重复的 ShadowDoc Core 0.1 微基准。
//!
//! 这里不测 UI，也不假装等同于真实用户研究。它只回答四个可机械验证的问题：
//! 1. 同一输入重复解析的锚点是否稳定；
//! 2. 只发送选中 unit + 前后文，相对发送全部 units 能少多少 UTF-8 字节；
//! 3. 解析耗时的量级；
//! 4. 原件变化后，旧 ShadowDoc 是否会被核心拒绝。

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use redline_core::inspect::{self, Snapshot, Unit};
use serde::Serialize;
use serde_json::json;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

const REPETITIONS: usize = 10;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Scenario {
    name: String,
    format: String,
    corpus: String,
    source_bytes: u64,
    units: usize,
    selected_unit_ids: Vec<String>,
    full_context_bytes: usize,
    selected_context_bytes: usize,
    context_reduction_percent: f64,
    anchor_determinism_percent: f64,
    inspect_median_ms: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SafetyCheck {
    name: &'static str,
    passed: bool,
    expected_error_code: &'static str,
    actual_error_code: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema: &'static str,
    repetitions: usize,
    context_metric: &'static str,
    scenarios: Vec<Scenario>,
    safety_checks: Vec<SafetyCheck>,
}

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> std::io::Result<Self> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
        let path = std::env::temp_dir().join(format!("redline-shadowdoc-benchmark-{}-{nonce}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let safe_name =
            self.path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("redline-shadowdoc-benchmark-"));
        if safe_name && self.path.parent() == Some(std::env::temp_dir().as_path()) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (corpus_dir, _corpus_scratch) = corpus_dir_from_args()?;
    let safety_scratch = Scratch::new()?;
    let docx = corpus_dir.join("controlled.docx");
    let xlsx = corpus_dir.join("controlled.xlsx");
    let pptx = corpus_dir.join("controlled.pptx");
    let pdf = corpus_dir.join("controlled.pdf");

    write_docx(&docx, 200)?;
    write_xlsx(&xlsx, 24, 100)?;
    write_pptx(&pptx, 40)?;
    write_pdf(&pdf, 40)?;

    let mut scenarios = vec![
        benchmark("controlled-docx", "controlled generated corpus", &docx)?,
        benchmark("controlled-xlsx", "controlled generated corpus", &xlsx)?,
        benchmark("controlled-pptx", "controlled generated corpus", &pptx)?,
        benchmark("controlled-pdf", "controlled generated corpus", &pdf)?,
    ];

    let real_markdown = Path::new("docs").join("影文档协议.md");
    if real_markdown.is_file() {
        scenarios.push(benchmark("repository-shadowdoc-markdown", "real repository document", &real_markdown)?);
    }

    let safety_checks = vec![stale_source_check(&safety_scratch.path)?];
    let report = Report {
        schema: "redline.shadowdoc-benchmark/0.1",
        repetitions: REPETITIONS,
        context_metric: "UTF-8 bytes of JSON-serialized units; selected window is target unit plus one neighbor on each side",
        scenarios,
        safety_checks,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn corpus_dir_from_args() -> Result<(PathBuf, Option<Scratch>), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [] => {
            let scratch = Scratch::new()?;
            Ok((scratch.path.clone(), Some(scratch)))
        }
        [flag, path] if flag == "--corpus-dir" => {
            let raw = PathBuf::from(path);
            let path = if raw.is_absolute() { raw } else { std::env::current_dir()?.join(raw) };
            fs::create_dir_all(&path)?;
            Ok((path, None))
        }
        _ => Err("用法: shadowdoc_benchmark [--corpus-dir PATH]".into()),
    }
}

fn benchmark(name: &str, corpus: &str, path: &Path) -> Result<Scenario, Box<dyn std::error::Error>> {
    let path_text = path.to_string_lossy();
    let baseline = inspect::inspect(&path_text)?;
    let anchors = anchor_signature(&baseline);
    let mut stable = 0usize;
    let mut durations_ns = Vec::with_capacity(REPETITIONS);

    for _ in 0..REPETITIONS {
        let start = Instant::now();
        let current = inspect::inspect(&path_text)?;
        durations_ns.push(start.elapsed().as_nanos());
        if current.source.sha256 == baseline.source.sha256 && anchor_signature(&current) == anchors {
            stable += 1;
        }
    }
    durations_ns.sort_unstable();
    let median_ns = durations_ns[durations_ns.len() / 2];

    let selected = selected_window(&baseline.units);
    let full_context_bytes = serde_json::to_vec(&baseline.units)?.len();
    let selected_context_bytes = serde_json::to_vec(&selected)?.len();
    let reduction =
        if full_context_bytes == 0 { 0.0 } else { round2((1.0 - selected_context_bytes as f64 / full_context_bytes as f64) * 100.0) };

    Ok(Scenario {
        name: name.into(),
        format: baseline.format,
        corpus: corpus.into(),
        source_bytes: baseline.source.bytes,
        units: baseline.units.len(),
        selected_unit_ids: selected.iter().map(|unit| unit.id.clone()).collect(),
        full_context_bytes,
        selected_context_bytes,
        context_reduction_percent: reduction,
        anchor_determinism_percent: round2(stable as f64 / REPETITIONS as f64 * 100.0),
        inspect_median_ms: round3(median_ns as f64 / 1_000_000.0),
    })
}

fn anchor_signature(snapshot: &Snapshot) -> Vec<(&str, &str, &str)> {
    snapshot.units.iter().map(|unit| (unit.id.as_str(), unit.kind.as_str(), unit.text_sha256.as_str())).collect()
}

fn selected_window(units: &[Unit]) -> Vec<&Unit> {
    if units.len() <= 3 {
        return units.iter().collect();
    }
    let center = units.len() / 2;
    units[center - 1..=center + 1].iter().collect()
}

fn stale_source_check(dir: &Path) -> Result<SafetyCheck, Box<dyn std::error::Error>> {
    let source = dir.join("stale-source.txt");
    fs::write(&source, b"before")?;
    let old_hash = inspect::inspect(&source.to_string_lossy())?.source.sha256;
    OpenOptions::new().append(true).open(&source)?.write_all(b"-after")?;

    let output = redline_core::dispatch(
        "agent.dispatch",
        &json!({
            "agent": "codex",
            "source": source.to_string_lossy(),
            "expectedSourceSha256": old_hash,
            "output": dir.join("stale-result.txt").to_string_lossy(),
            "annotations": [],
            "dryRun": true
        }),
    );
    let actual = output.pointer("/error/code").and_then(|value| value.as_str()).map(str::to_string);
    Ok(SafetyCheck {
        name: "old ShadowDoc is refused after source bytes change",
        passed: output["ok"] == false && actual.as_deref() == Some("stale_shadow"),
        expected_error_code: "stale_shadow",
        actual_error_code: actual,
    })
}

fn write_zip(path: &Path, entries: Vec<(String, Vec<u8>)>) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(path)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, body) in entries {
        writer.start_file(name, options)?;
        writer.write_all(&body)?;
    }
    writer.finish()?;
    Ok(())
}

fn write_docx(path: &Path, paragraphs: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
    );
    for index in 1..=paragraphs {
        xml.push_str(&format!(
            "<w:p><w:r><w:t>Section {index}: controlled benchmark content for human annotation and bounded AI editing.</w:t></w:r></w:p>"
        ));
    }
    xml.push_str("</w:body></w:document>");
    write_zip(
        path,
        vec![
            (
                "[Content_Types].xml".into(),
                br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_vec(),
            ),
            (
                "_rels/.rels".into(),
                br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_vec(),
            ),
            ("word/document.xml".into(), xml.into_bytes()),
        ],
    )
}

fn write_xlsx(path: &Path, sheets: usize, cells_per_sheet: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut workbook = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>"#,
    );
    let mut content_types = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>"#,
    );
    let mut workbook_rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    let mut entries = Vec::with_capacity(sheets + 4);
    for sheet in 1..=sheets {
        workbook.push_str(&format!(r#"<sheet name="Sheet {sheet}" sheetId="{sheet}" r:id="rId{sheet}"/>"#));
        workbook_rels.push_str(&format!(
            r#"<Relationship Id="rId{sheet}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{sheet}.xml"/>"#
        ));
        content_types.push_str(&format!(
            r#"<Override PartName="/xl/worksheets/sheet{sheet}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        ));
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
        );
        for row in 1..=cells_per_sheet {
            xml.push_str(&format!(
                r#"<row r="{row}"><c r="A{row}" t="inlineStr"><is><t>Sheet {sheet} row {row} controlled value</t></is></c><c r="B{row}"><v>{}</v></c></row>"#,
                sheet * row
            ));
        }
        xml.push_str("</sheetData></worksheet>");
        entries.push((format!("xl/worksheets/sheet{sheet}.xml"), xml.into_bytes()));
    }
    workbook.push_str("</sheets></workbook>");
    workbook_rels.push_str("</Relationships>");
    content_types.push_str("</Types>");
    entries.push(("[Content_Types].xml".into(), content_types.into_bytes()));
    entries.push((
        "_rels/.rels".into(),
        br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_vec(),
    ));
    entries.push(("xl/_rels/workbook.xml.rels".into(), workbook_rels.into_bytes()));
    entries.push(("xl/workbook.xml".into(), workbook.into_bytes()));
    write_zip(path, entries)
}

fn write_pptx(path: &Path, slides: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut content_types = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>"#,
    );
    let mut presentation = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>"#,
    );
    let mut presentation_rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#,
    );
    let mut entries = Vec::with_capacity(slides * 2 + 8);
    for slide in 1..=slides {
        let relationship_id = slide + 1;
        presentation.push_str(&format!(r#"<p:sldId id="{}" r:id="rId{relationship_id}"/>"#, 255 + slide));
        presentation_rels.push_str(&format!(
            r#"<Relationship Id="rId{relationship_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{slide}.xml"/>"#
        ));
        content_types.push_str(&format!(
            r#"<Override PartName="/ppt/slides/slide{slide}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="TextBox {slide}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Slide {slide} controlled benchmark content for review and revision.</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
        );
        entries.push((format!("ppt/slides/slide{slide}.xml"), xml.into_bytes()));
        entries.push((
            format!("ppt/slides/_rels/slide{slide}.xml.rels"),
            br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#.to_vec(),
        ));
    }
    presentation.push_str(r#"</p:sldIdLst><p:sldSz cx="9144000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#);
    presentation_rels.push_str("</Relationships>");
    content_types.push_str("</Types>");
    entries.push(("[Content_Types].xml".into(), content_types.into_bytes()));
    entries.push((
        "_rels/.rels".into(),
        br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#.to_vec(),
    ));
    entries.push(("ppt/presentation.xml".into(), presentation.into_bytes()));
    entries.push(("ppt/_rels/presentation.xml.rels".into(), presentation_rels.into_bytes()));
    entries.push((
        "ppt/slideLayouts/slideLayout1.xml".into(),
        br#"<?xml version="1.0" encoding="UTF-8"?><p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" type="blank"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#.to_vec(),
    ));
    entries.push((
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
        br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#.to_vec(),
    ));
    entries.push((
        "ppt/slideMasters/slideMaster1.xml".into(),
        br#"<?xml version="1.0" encoding="UTF-8"?><p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMap accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" bg1="lt1" bg2="lt2" folHlink="folHlink" hlink="hlink" tx1="dk1" tx2="dk2"/><p:sldLayoutIdLst><p:sldLayoutId id="1" r:id="rId1"/></p:sldLayoutIdLst></p:sldMaster>"#.to_vec(),
    ));
    entries.push((
        "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
        br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#.to_vec(),
    ));
    write_zip(path, entries)
}

fn write_pdf(path: &Path, pages: usize) -> Result<(), Box<dyn std::error::Error>> {
    let font_id = 3 + pages * 2;
    let mut objects = vec![Vec::<u8>::new(); font_id + 1];
    objects[1] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();

    let mut kids = String::new();
    for page in 0..pages {
        let page_id = 3 + page * 2;
        let content_id = page_id + 1;
        kids.push_str(&format!("{page_id} 0 R "));
        objects[page_id] = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 {font_id} 0 R >> >> /Contents {content_id} 0 R >>"
        )
        .into_bytes();
        let stream = format!("BT /F1 12 Tf 72 720 Td (Page {} controlled benchmark content for review.) Tj ET", page + 1);
        objects[content_id] = format!("<< /Length {} >>\nstream\n{}\nendstream", stream.len(), stream).into_bytes();
    }
    objects[2] = format!("<< /Type /Pages /Count {pages} /Kids [{kids}] >>").into_bytes();
    objects[font_id] = b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec();

    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0usize; objects.len()];
    for id in 1..objects.len() {
        offsets[id] = pdf.len();
        pdf.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        pdf.extend_from_slice(&objects[id]);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len()).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len()).as_bytes());
    fs::write(path, pdf)?;
    Ok(())
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}
