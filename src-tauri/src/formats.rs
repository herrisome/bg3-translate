//! 三种格式（XML contentList / LSX / LOCA）与统一 TranslationEntry 的互转。
//!
//! 设计要点（已通过阶段 0 验证）：
//! - contentuid / version 是句柄，**绝不能改**，只改文本
//! - 保留富文本标签（<LSTag>、<font>）和占位符（{1}、{2}）
//! - LOCA 通过 bg3rustpaklib::loca 模块读写（已验证往返无损）

use std::path::Path;

use bg3rustpaklib::loca::{LocaFormat, LocaResource, LocaUtils};

use crate::error::{AppError, Result};
use crate::types::{PakFileKind, TranslationEntry};

/// 从磁盘文件读取可翻译条目。按文件类型分发。
pub fn read_entries(
    work_dir: &str,
    file_name: &str,
    kind: &PakFileKind,
) -> Result<Vec<TranslationEntry>> {
    let path = crate::pak::resolve_disk_path(work_dir, file_name);
    if !path.exists() {
        return Err(AppError::Config(format!(
            "文件不存在: {}",
            path.display()
        )));
    }
    match kind {
        PakFileKind::LocalizationXml => read_content_list_xml(&path, file_name),
        PakFileKind::LocalizationLoca => read_loca(&path, file_name),
        PakFileKind::MetadataLsx => read_lsx(&path, file_name),
        _ => Ok(vec![]),
    }
}

/// 把编辑后的条目写回磁盘文件（保持原格式）。
pub fn write_entries(
    work_dir: &str,
    file_name: &str,
    kind: &PakFileKind,
    entries: &[TranslationEntry],
) -> Result<()> {
    let path = crate::pak::resolve_disk_path(work_dir, file_name);
    match kind {
        PakFileKind::LocalizationXml => write_content_list_xml(&path, entries),
        PakFileKind::LocalizationLoca => write_loca(&path, entries),
        PakFileKind::MetadataLsx => write_lsx(&path, entries),
        _ => Ok(()),
    }
}

// ─────────────────────────────────────────────────────────────
// contentList XML
// ─────────────────────────────────────────────────────────────
//
// 格式：
//   <contentList>
//     <content contentuid="h..." version="1">文本</content>
//   </contentList>

fn read_content_list_xml(
    path: &Path,
    file_name: &str,
) -> Result<Vec<TranslationEntry>> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_content_list(&content, file_name))
}

/// 解析 contentList XML 文本，提取所有 <content> 条目。
///
/// 注意：`<content>` 内部可能包含富文本子标签（如 `<LSTag>`、`<font>`），
/// 这些必须原样保留。因此当进入 `<content>` 后，我们把所有内部事件
/// （文本 + 子标签）累积为原始 XML 字符串，而不做反序列化。
pub fn parse_content_list(xml: &str, file_name: &str) -> Vec<TranslationEntry> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut entries = Vec::new();
    let mut buf = Vec::new();
    // 当前正在处理的 <content> 的属性
    let mut pending: Option<(String, String)> = None; // (contentuid, version)
    let mut raw = String::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"content" && pending.is_none() => {
                let mut contentuid = String::new();
                let mut version = String::new();
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"contentuid" => {
                            contentuid =
                                attr.unescape_value().unwrap_or_default().into_owned();
                        }
                        b"version" => {
                            version =
                                attr.unescape_value().unwrap_or_default().into_owned();
                        }
                        _ => {}
                    }
                }
                pending = Some((contentuid, version));
                raw.clear();
            }
            Ok(Event::Empty(e)) if e.name().as_ref() == b"content" && pending.is_none() => {
                // 自闭合 <content .../>，无文本
                let mut contentuid = String::new();
                let mut version = String::new();
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"contentuid" => {
                            contentuid =
                                attr.unescape_value().unwrap_or_default().into_owned();
                        }
                        b"version" => {
                            version =
                                attr.unescape_value().unwrap_or_default().into_owned();
                        }
                        _ => {}
                    }
                }
                entries.push(make_entry(file_name, contentuid, version, String::new()));
            }
            // 在 content 内部：累积所有事件为原始 XML
            Ok(Event::Start(ref e)) if pending.is_some() => {
                raw.push_str(&event_to_raw_start(e));
            }
            Ok(Event::End(e)) if pending.is_some() && e.name().as_ref() == b"content" => {
                // content 结束
                if let Some((contentuid, version)) = pending.take() {
                    let source = unescape_keep_tags(raw.trim());
                    entries.push(make_entry(file_name, contentuid, version, source));
                }
                raw.clear();
            }
            Ok(Event::End(ref e)) if pending.is_some() => {
                raw.push_str(&event_to_raw_end(e));
            }
            Ok(Event::Empty(ref e)) if pending.is_some() => {
                raw.push_str(&event_to_raw_empty(e));
            }
            Ok(Event::Text(e)) if pending.is_some() => {
                raw.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::CData(e)) if pending.is_some() => {
                raw.push_str(&String::from_utf8_lossy(e.as_ref()));
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                log::warn!("XML 解析警告: {e}");
                break;
            }
            _ => {}
        }
    }
    entries
}

/// 把 Start 事件转回原始 XML 字符串（含属性）。
fn event_to_raw_start(e: &quick_xml::events::BytesStart) -> String {
    let mut s = String::from("<");
    s.push_str(std::str::from_utf8(e.name().as_ref()).unwrap_or(""));
    for attr in e.attributes().flatten() {
        s.push(' ');
        s.push_str(std::str::from_utf8(attr.key.as_ref()).unwrap_or(""));
        s.push_str("=\"");
        s.push_str(&attr.unescape_value().unwrap_or_default());
        s.push('"');
    }
    s.push('>');
    s
}

fn event_to_raw_end(e: &quick_xml::events::BytesEnd) -> String {
    let mut s = String::from("</");
    s.push_str(std::str::from_utf8(e.name().as_ref()).unwrap_or(""));
    s.push('>');
    s
}

fn event_to_raw_empty(e: &quick_xml::events::BytesStart) -> String {
    let mut s = event_to_raw_start(e);
    // 空元素自闭合
    if s.ends_with('>') {
        s.pop();
        s.push_str("/>");
    }
    s
}

/// contentList 的文本节点已被 quick-xml 自动 unescape（Text 事件），
/// 但内部子标签是我们手动重组的，无需再次 unescape。
fn unescape_keep_tags(s: &str) -> String {
    s.to_string()
}

fn make_entry(
    file_name: &str,
    contentuid: String,
    version: String,
    source: String,
) -> TranslationEntry {
    TranslationEntry {
        id: format!("{file_name}#{contentuid}"),
        source_file: file_name.to_string(),
        source,
        target: String::new(),
        contentuid,
        version,
        status: "pending".into(),
        error: None,
    }
}

fn write_content_list_xml(path: &Path, entries: &[TranslationEntry]) -> Result<()> {
    use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
    use quick_xml::Writer;
    use std::io::Cursor;

    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
        .map_err(|e| AppError::Xml(format!("{e}")))?;
    writer
        .write_event(Event::Start(BytesStart::new("contentList")))
        .map_err(|e| AppError::Xml(format!("{e}")))?;

    for e in entries {
        let mut start = BytesStart::new("content");
        start.push_attribute(("contentuid", e.contentuid.as_str()));
        start.push_attribute(("version", e.version.as_str()));
        writer
            .write_event(Event::Start(start))
            .map_err(|err| AppError::Xml(format!("{err}")))?;
        // 优先用译文；若译文空则保留原文
        let text = if e.target.is_empty() { &e.source } else { &e.target };
        writer
            .write_event(Event::Text(BytesText::new(text)))
            .map_err(|err| AppError::Xml(format!("{err}")))?;
        writer
            .write_event(Event::End(BytesEnd::new("content")))
            .map_err(|err| AppError::Xml(format!("{err}")))?;
    }
    writer
        .write_event(Event::End(BytesEnd::new("contentList")))
        .map_err(|e| AppError::Xml(format!("{e}")))?;

    let result = writer.into_inner().into_inner();
    std::fs::write(path, result)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// LOCA 二进制（通过 bg3rustpaklib::loca）
// ─────────────────────────────────────────────────────────────

fn read_loca(path: &Path, file_name: &str) -> Result<Vec<TranslationEntry>> {
    let resource = LocaUtils::load(path)?;
    Ok(loca_resource_to_entries(resource, file_name))
}

fn write_loca(path: &Path, entries: &[TranslationEntry]) -> Result<()> {
    let resource = entries_to_loca_resource(entries);
    LocaUtils::save_with_format(&resource, path, LocaFormat::Loca)?;
    Ok(())
}

fn loca_resource_to_entries(
    resource: LocaResource,
    file_name: &str,
) -> Vec<TranslationEntry> {
    resource
        .entries
        .into_iter()
        .map(|t| TranslationEntry {
            id: format!("{file_name}#{}", t.key),
            source_file: file_name.to_string(),
            source: t.text,
            target: String::new(),
            contentuid: t.key,
            version: t.version.to_string(),
            status: "pending".into(),
            error: None,
        })
        .collect()
}

fn entries_to_loca_resource(entries: &[TranslationEntry]) -> LocaResource {
    use bg3rustpaklib::loca::LocalizedText;
    let mapped = entries
        .iter()
        .map(|e| {
            let text = if e.target.is_empty() { &e.source } else { &e.target };
            LocalizedText::new(
                e.contentuid.clone(),
                e.version.parse().unwrap_or(1),
                text.clone(),
            )
        })
        .collect();
    LocaResource::with_entries(mapped)
}

// ─────────────────────────────────────────────────────────────
// LSX 元数据
// ─────────────────────────────────────────────────────────────
//
// 纯文本 XML。按字段白名单提取可翻译 attribute：
//   Description / DisplayName / Name / Title / Tooltip
// 且 type 为 LSString / LSWString。
// 翻译后写回，保留原 XML 结构和 BOM。

/// LSX 中可翻译的 attribute id 白名单。
const LSX_TRANSLATABLE_FIELDS: &[&str] =
    &["Description", "DisplayName", "Title", "Tooltip", "TooltipDescription"];

fn read_lsx(path: &Path, file_name: &str) -> Result<Vec<TranslationEntry>> {
    let bytes = std::fs::read(path)?;
    let content = strip_bom(&bytes);
    let xml = String::from_utf8(content)
        .map_err(|e| AppError::Xml(format!("LSX 非 UTF-8: {e}")))?;
    Ok(parse_lsx_translatable(&xml, file_name))
}

fn write_lsx(path: &Path, entries: &[TranslationEntry]) -> Result<()> {
    // 策略：读取原文件，对有译文的条目原地替换 attribute value，保留其他结构。
    let bytes = std::fs::read(path)?;
    let has_bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
    let mut content = String::from_utf8(strip_bom(&bytes))
        .map_err(|e| AppError::Xml(format!("LSX 非 UTF-8: {e}")))?;

    for e in entries {
        if e.target.is_empty() {
            continue; // 没译文的跳过，保留原文
        }
        if let Some((field, occurrence)) = decode_lsx_id(&e.contentuid) {
            replace_nth_lsx_value(&mut content, &field, occurrence, &e.target);
        }
    }

    let mut out = Vec::new();
    if has_bom {
        out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    out.extend_from_slice(content.as_bytes());
    std::fs::write(path, out)?;
    Ok(())
}

fn strip_bom(bytes: &[u8]) -> Vec<u8> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes[3..].to_vec()
    } else {
        bytes.to_vec()
    }
}

/// 解析 LSX，收集可翻译的 attribute。
/// contentuid 编码为 `{field}#{occurrence}` 以便写回时定位。
fn parse_lsx_translatable(xml: &str, file_name: &str) -> Vec<TranslationEntry> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut entries = Vec::new();
    let mut occurrence: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e))
                if e.name().as_ref() == b"attribute" =>
            {
                let mut id = String::new();
                let mut typ = String::new();
                let mut value = String::new();
                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"id" => {
                            id = attr.unescape_value().unwrap_or_default().into_owned()
                        }
                        b"type" => {
                            typ = attr.unescape_value().unwrap_or_default().into_owned()
                        }
                        b"value" => {
                            value = attr.unescape_value().unwrap_or_default().into_owned()
                        }
                        _ => {}
                    }
                }
                let is_text_type = typ == "LSString" || typ == "LSWString";
                if LSX_TRANSLATABLE_FIELDS.contains(&id.as_str())
                    && is_text_type
                    && !value.is_empty()
                {
                    let occ = occurrence.entry(id.clone()).or_insert(0);
                    let contentuid = format!("{id}#{occ}");
                    *occ += 1;
                    entries.push(TranslationEntry {
                        id: format!("{file_name}#{contentuid}"),
                        source_file: file_name.to_string(),
                        source: value,
                        target: String::new(),
                        contentuid,
                        version: "1".into(),
                        status: "pending".into(),
                        error: None,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                log::warn!("LSX 解析警告: {e}");
                break;
            }
            _ => {}
        }
    }
    entries
}

fn decode_lsx_id(id: &str) -> Option<(String, usize)> {
    let mut parts = id.split('#');
    let field = parts.next()?.to_string();
    let occ: usize = parts.next()?.parse().ok()?;
    Some((field, occ))
}

/// 替换 LSX 中第 n 次出现的 `id="{field}"` 之后最近的 `value="..."`。
fn replace_nth_lsx_value(content: &mut String, field: &str, n: usize, new_value: &str) {
    let needle = format!("id=\"{field}\"");
    let mut count = 0;
    let mut search_from = 0;

    while search_from < content.len() {
        if let Some(pos) = content[search_from..].find(&needle) {
            let abs = search_from + pos;
            let rest = &content[abs..];
            if let Some(vpos) = rest.find("value=\"") {
                let v_start = abs + vpos + "value=\"".len();
                if let Some(end) = find_attr_end(&content[v_start..]).map(|i| v_start + i) {
                    if count == n {
                        let escaped = escape_xml_attr(new_value);
                        content.replace_range(v_start..end, &escaped);
                        return;
                    }
                    count += 1;
                }
            }
            search_from = abs + needle.len();
        } else {
            break;
        }
    }
}

fn find_attr_end(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            return Some(i);
        }
        if bytes[i] == b'&' {
            while i < bytes.len() && bytes[i] != b';' {
                i += 1;
            }
        }
        i += 1;
    }
    None
}

fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_list_basic() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<contentList>
  <content contentuid="habc123" version="1">Hello</content>
  <content contentuid="hdef456" version="1">World</content>
</contentList>"#;
        let entries = parse_content_list(xml, "test.xml");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].contentuid, "habc123");
        assert_eq!(entries[0].source, "Hello");
        assert_eq!(entries[1].source, "World");
    }

    #[test]
    fn parse_content_list_preserves_tags() {
        // 富文本标签应保留在文本中
        let xml = r#"<contentList>
  <content contentuid="h1" version="1">Cast <LSTag Tag="Fire">Fireball</LSTag> for {1} damage</content>
</contentList>"#;
        let entries = parse_content_list(xml, "test.xml");
        assert_eq!(entries.len(), 1);
        assert!(entries[0].source.contains("<LSTag"));
        assert!(entries[0].source.contains("{1}"));
    }
}
