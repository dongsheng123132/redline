#!/usr/bin/env python3
"""⚠️ 已废弃 —— 请改用 `redline.exe`（crates/redline-cli）。

这是 Rust 动作核心之前的第一版 CLI。它和 TS 渲染层各自实现了一遍文档解析，
正是本项目要消灭的那种漂移；而且它要求目标机器装了 Python。
功能已全部迁进 crates/redline-core，本文件仅供对照，新代码不要再引用。

原始说明：Redline CLI — AI 文档协作的安全动作核心。

默认只读：inspect / diff 只输出 JSON。
apply 只支持 .docx 的唯一单文本节点替换，写入原生 Word Track Changes，
并且要求一个不同于原件的新输出路径。
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import zipfile
from datetime import UTC, datetime
from pathlib import Path
from typing import Any
from xml.etree import ElementTree as ET
from xml.sax.saxutils import escape


VERSION = "0.1.0"
WORD_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
SHEET_NS = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
PPT_NS = "http://schemas.openxmlformats.org/drawingml/2006/main"
NS = {"w": WORD_NS, "s": SHEET_NS, "a": PPT_NS}
ACTION_INSPECT = "document.inspect"
ACTION_DIFF = "document.diff"
ACTION_APPLY_TRACK_CHANGES = "document.apply-track-changes"
ACTION_VERIFY = "document.verify"


def fail(message: str, details: dict[str, Any] | None = None) -> None:
    payload: dict[str, Any] = {"ok": False, "error": message}
    if details:
        payload["details"] = details
    print(json.dumps(payload, ensure_ascii=False, indent=2), file=sys.stderr)
    raise SystemExit(1)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def local_path(value: str) -> Path:
    return Path(value).expanduser().resolve()


def read_zip(path: Path) -> tuple[list[zipfile.ZipInfo], dict[str, bytes]]:
    with zipfile.ZipFile(path, "r") as archive:
        infos = archive.infolist()
        return infos, {info.filename: archive.read(info.filename) for info in infos}


def require_part(parts: dict[str, bytes], name: str) -> str:
    if name not in parts:
        raise ValueError(f"缺少 OOXML 部件：{name}")
    return parts[name].decode("utf-8")


def inspect_docx(source: Path, data: bytes) -> dict[str, Any]:
    _, parts = read_zip(source)
    document_xml = require_part(parts, "word/document.xml")
    root = ET.fromstring(document_xml)
    paragraphs: list[str] = []
    for paragraph in root.findall(".//w:p", NS):
        text = "".join(node.text or "" for node in paragraph.findall(".//w:t", NS)).strip()
        if text:
            paragraphs.append(text)
    return {
        "format": "docx",
        "summary": {
            "paragraphs": len(paragraphs),
            "tables": len(root.findall(".//w:tbl", NS)),
            "images": sum(name.startswith("word/media/") for name in parts),
            "trackedInsertions": document_xml.count("<w:ins"),
            "trackedDeletions": document_xml.count("<w:del"),
        },
        "units": [
            {"id": f"paragraph:{index}", "label": f"段落 {index}", "text": text}
            for index, text in enumerate(paragraphs, start=1)
        ],
    }


def inspect_pdf(data: bytes) -> dict[str, Any]:
    try:
        from pypdf import PdfReader  # type: ignore[import-not-found]
    except ImportError as exc:
        raise ValueError("此 Python 环境没有 pypdf；PDF 只读解析需要安装 pypdf。") from exc
    from io import BytesIO

    reader = PdfReader(BytesIO(data))
    units = []
    for index, page in enumerate(reader.pages, start=1):
        box = page.mediabox
        units.append(
            {
                "id": f"page:{index}",
                "label": f"第 {index} 页",
                "text": (page.extract_text() or "").replace("\x00", "").strip(),
                "renderSize": {"width": float(box.width), "height": float(box.height)},
            }
        )
    return {"format": "pdf", "summary": {"pages": len(units)}, "units": units}


def inspect_xlsx(source: Path, data: bytes) -> dict[str, Any]:
    _, parts = read_zip(source)
    shared_strings: list[str] = []
    if "xl/sharedStrings.xml" in parts:
        root = ET.fromstring(parts["xl/sharedStrings.xml"])
        for item in root.findall("s:si", NS):
            shared_strings.append("".join(node.text or "" for node in item.findall(".//s:t", NS)))
    workbook = ET.fromstring(require_part(parts, "xl/workbook.xml"))
    sheet_names = [node.attrib.get("name", f"Sheet {index}") for index, node in enumerate(workbook.findall(".//s:sheet", NS), start=1)]
    units = []
    for index, name in enumerate(sheet_names, start=1):
        sheet_name = f"xl/worksheets/sheet{index}.xml"
        if sheet_name not in parts:
            units.append({"id": f"sheet:{index}", "label": f"工作表 {name}", "text": "", "note": "未找到工作表 XML"})
            continue
        root = ET.fromstring(parts[sheet_name])
        cells = []
        for cell in root.findall(".//s:c", NS):
            address = cell.attrib.get("r", "?")
            kind = cell.attrib.get("t")
            value = cell.findtext("s:v", default="", namespaces=NS)
            formula = cell.findtext("s:f", default="", namespaces=NS)
            if kind == "s" and value.isdigit() and int(value) < len(shared_strings):
                value = shared_strings[int(value)]
            elif kind == "inlineStr":
                value = "".join(node.text or "" for node in cell.findall(".//s:t", NS))
            display = f"{address}={value}"
            if formula:
                display += f" [公式:{formula}]"
            cells.append(display)
        units.append({"id": f"sheet:{index}", "label": f"工作表 {name}", "text": "\n".join(cells)})
    return {"format": "xlsx", "summary": {"sheets": len(units)}, "units": units}


def inspect_pptx(source: Path, data: bytes) -> dict[str, Any]:
    _, parts = read_zip(source)
    slide_names = sorted(
        (name for name in parts if re.fullmatch(r"ppt/slides/slide\d+\.xml", name)),
        key=lambda name: int(re.search(r"slide(\d+)", name).group(1)),
    )
    units = []
    for index, name in enumerate(slide_names, start=1):
        root = ET.fromstring(parts[name])
        text = " ".join((node.text or "").strip() for node in root.findall(".//a:t", NS) if (node.text or "").strip())
        units.append({"id": f"slide:{index}", "label": f"幻灯片 {index}", "text": text})
    return {"format": "pptx", "summary": {"slides": len(units)}, "units": units}


def inspect_file(value: str) -> dict[str, Any]:
    source = local_path(value)
    if not source.is_file():
        raise ValueError(f"文件不存在：{source}")
    data = source.read_bytes()
    extension = source.suffix.lower()
    if extension == ".docx":
        body = inspect_docx(source, data)
    elif extension == ".pdf":
        body = inspect_pdf(data)
    elif extension == ".xlsx":
        body = inspect_xlsx(source, data)
    elif extension == ".pptx":
        body = inspect_pptx(source, data)
    elif extension in {".txt", ".md", ".html", ".htm"}:
        body = {"format": extension[1:], "summary": {"units": 1}, "units": [{"id": "document:1", "label": "文档", "text": data.decode("utf-8", errors="replace")}]}
    elif extension == ".doc":
        raise ValueError("旧版 .doc 是二进制格式：Redline 不会伪造“无损可编辑”。请保留原件后用 Word/WPS/LibreOffice 另存为 .docx，再执行 inspect/apply。")
    else:
        raise ValueError(f"暂不支持 {extension or '无扩展名'}；支持 .docx/.pdf/.xlsx/.pptx/.txt/.md/.html")
    return {
        "version": 1,
        "tool": f"redline-cli/{VERSION}",
        "action_id": ACTION_INSPECT,
        "source": {"path": str(source), "extension": extension, "bytes": len(data), "sha256": sha256(data)},
        **body,
    }


def diff_snapshots(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    if before["format"] != after["format"]:
        fail("只能对比同一格式的文件", {"before": before["format"], "after": after["format"]})
    left = {unit["id"]: unit for unit in before["units"]}
    right = {unit["id"]: unit for unit in after["units"]}
    changes = []
    for unit_id in sorted(set(left) | set(right)):
        old = left.get(unit_id, {}).get("text", "")
        new = right.get(unit_id, {}).get("text", "")
        if old != new:
            unit = right.get(unit_id) or left.get(unit_id) or {}
            changes.append({"id": unit_id, "label": unit.get("label", unit_id), "before": old, "after": new})
    return {
        "version": 1,
        "tool": f"redline-cli/{VERSION}",
        "action_id": ACTION_DIFF,
        "format": before["format"],
        "before": before["source"],
        "after": after["source"],
        "summary": {"changedUnits": len(changes), "unchangedUnits": len(set(left) | set(right)) - len(changes)},
        "changes": changes,
    }


def next_revision_id(document_xml: str) -> int:
    ids = [int(value) for value in re.findall(r'w:id="(\d+)"', document_xml)]
    return max(ids, default=0) + 1


def replace_single_run(document_xml: str, old: str, new: str, revision_id: int, author: str, date: str) -> str:
    xml_entities = {'"': "&quot;", "'": "&apos;"}
    old_xml = escape(old, xml_entities)
    new_xml = escape(new, xml_entities)
    pattern = re.compile(
        r"<w:r\b[^>]*>(?:(?!</w:r>).)*?<w:t\b[^>]*>" + re.escape(old_xml) + r"</w:t>(?:(?!</w:r>).)*?</w:r>",
        re.DOTALL,
    )
    matches = list(pattern.finditer(document_xml))
    if len(matches) != 1:
        raise ValueError(f"替换项“{old[:80]}”必须在一个 Word 文本节点中唯一出现；实际匹配 {len(matches)} 次。请把 patch 写得更精确。")
    run = matches[0].group(0)
    if len(re.findall(r"<w:t\b", run)) != 1:
        raise ValueError("该替换命中一个含多个文本节点的 Word run；为避免破坏格式，CLI 拒绝自动修改。")
    deleted = re.sub(r"<w:t\b([^>]*)>[\s\S]*?</w:t>", lambda item: f"<w:delText{item.group(1)}>{old_xml}</w:delText>", run, count=1)
    inserted = re.sub(r"<w:t\b([^>]*)>[\s\S]*?</w:t>", lambda item: f"<w:t{item.group(1)}>{new_xml}</w:t>", run, count=1)
    metadata = f' w:id="{revision_id}" w:author="{escape(author, xml_entities)}" w:date="{date}"'
    revision = f"<w:del{metadata}>{deleted}</w:del><w:ins{metadata}>{inserted}</w:ins>"
    return document_xml[: matches[0].start()] + revision + document_xml[matches[0].end() :]


def ensure_track_revisions(parts: dict[str, bytes]) -> None:
    name = "word/settings.xml"
    if name not in parts:
        return
    settings = parts[name].decode("utf-8")
    if "<w:trackRevisions" not in settings:
        parts[name] = settings.replace("</w:settings>", "<w:trackRevisions/></w:settings>").encode("utf-8")


def write_zip(path: Path, infos: list[zipfile.ZipInfo], parts: dict[str, bytes]) -> None:
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for info in infos:
            clone = zipfile.ZipInfo(info.filename, date_time=info.date_time)
            clone.external_attr = info.external_attr
            clone.comment = info.comment
            clone.create_system = info.create_system
            clone.flag_bits = info.flag_bits
            archive.writestr(clone, parts[info.filename])


def apply_docx(input_value: str, patch_value: str, output_value: str, author: str, audit_value: str | None, force: bool) -> dict[str, Any]:
    source = local_path(input_value)
    output = local_path(output_value)
    if source.suffix.lower() != ".docx":
        fail("apply 当前只支持 .docx", {"input": str(source)})
    if source == output:
        fail("安全限制：输出路径不能等于输入路径。请保留原件并指定新文件。")
    if output.exists() and not force:
        fail("输出文件已存在；如确认覆盖该输出副本，请添加 --force", {"output": str(output)})
    patch = json.loads(local_path(patch_value).read_text(encoding="utf-8"))
    changes = patch.get("changes") if isinstance(patch, dict) else None
    if not isinstance(patch, dict) or patch.get("version") != 1 or not isinstance(changes, list) or not changes:
        fail("patch.json 必须含有 version: 1 和非空 changes 数组")
    if patch.get("action_id") not in {None, ACTION_APPLY_TRACK_CHANGES}:
        fail("patch.json 的 action_id 必须是 document.apply-track-changes", {"action_id": patch.get("action_id")})
    original = source.read_bytes()
    expected_hash = patch.get("expected_input_sha256")
    actual_hash = sha256(original)
    if expected_hash is not None and expected_hash != actual_hash:
        fail("原文件在计划后已变更，拒绝把陈旧 patch 写入新副本", {"expected_input_sha256": expected_hash, "actual_input_sha256": actual_hash})
    infos, parts = read_zip(source)
    document_xml = require_part(parts, "word/document.xml")
    date = datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    revision_id = next_revision_id(document_xml)
    applied = []
    for index, change in enumerate(changes):
        if not isinstance(change, dict) or change.get("op") != "replace_text" or not isinstance(change.get("old"), str) or not change["old"] or not isinstance(change.get("new"), str):
            fail(f"changes[{index}] 仅支持包含非空 old/new 的 replace_text")
        document_xml = replace_single_run(document_xml, change["old"], change["new"], revision_id, author, date)
        applied.append({"index": index, "op": "replace_text", "old": change["old"], "new": change["new"], "reason": change.get("reason"), "revisionId": revision_id})
        revision_id += 1
    parts["word/document.xml"] = document_xml.encode("utf-8")
    ensure_track_revisions(parts)
    output.parent.mkdir(parents=True, exist_ok=True)
    write_zip(output, infos, parts)
    output_data = output.read_bytes()
    audit = {
        "version": 1,
        "tool": f"redline-cli/{VERSION}",
        "action_id": ACTION_APPLY_TRACK_CHANGES,
        "input": {"path": str(source), "sha256": actual_hash},
        "output": {"path": str(output), "sha256": sha256(output_data)},
        "author": author,
        "createdAt": date,
        "applied": applied,
        "rollback": "删除输出文件即可回到原件；原输入文件未被修改。",
    }
    if audit_value:
        audit_path = local_path(audit_value)
        audit_path.parent.mkdir(parents=True, exist_ok=True)
        audit_path.write_text(json.dumps(audit, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return {"ok": True, **audit}


def write_json(value: dict[str, Any], output: str | None) -> None:
    content = json.dumps(value, ensure_ascii=False, indent=2) + "\n"
    if output:
        target = local_path(output)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")
    print(content, end="")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description="Redline — AI 文档协作 CLI（默认只读）")
    commands = root.add_subparsers(dest="command", required=True)
    inspect = commands.add_parser("inspect", help="本地解析并输出结构化快照")
    inspect.add_argument("file")
    inspect.add_argument("--out")
    diff = commands.add_parser("diff", help="对比两个同格式文档")
    diff.add_argument("before")
    diff.add_argument("after")
    diff.add_argument("--out")
    apply = commands.add_parser("apply", help="把显式 patch 写为新的 DOCX Track Changes")
    apply.add_argument("input")
    apply.add_argument("patch")
    apply.add_argument("output")
    apply.add_argument("--author", default="AI Redline")
    apply.add_argument("--audit")
    apply.add_argument("--force", action="store_true")
    verify = commands.add_parser("verify", help="检查文件是否可被 Redline 解析")
    verify.add_argument("file")
    return root


def main() -> None:
    args = parser().parse_args()
    try:
        if args.command == "inspect":
            write_json(inspect_file(args.file), args.out)
        elif args.command == "diff":
            write_json(diff_snapshots(inspect_file(args.before), inspect_file(args.after)), args.out)
        elif args.command == "apply":
            write_json(apply_docx(args.input, args.patch, args.output, args.author, args.audit, args.force), None)
        elif args.command == "verify":
            snapshot = inspect_file(args.file)
            write_json({"ok": True, "action_id": ACTION_VERIFY, "file": snapshot["source"], "format": snapshot["format"], "summary": snapshot["summary"]}, None)
    except (OSError, ValueError, zipfile.BadZipFile, ET.ParseError, json.JSONDecodeError) as exc:
        fail(str(exc))


if __name__ == "__main__":
    main()
