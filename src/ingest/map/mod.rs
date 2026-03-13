use std::fs;
use std::path::Path;

use crate::model::{
    ArchiveContribution, ArchivePullDetail, CrossReference, LinkerFamily, MapFormat, MapFormatSelection, MemoryRegion,
    ObjectContribution, ObjectSourceKind, ToolchainKind, ToolchainSelection, WarningItem, WarningLevel, WarningSource,
};

#[derive(Debug, Clone)]
pub struct MapIngestResult {
    pub detected_toolchain: Option<ToolchainKind>,
    pub resolved_toolchain: ToolchainKind,
    pub linker_family: LinkerFamily,
    pub map_format: MapFormat,
    pub object_contributions: Vec<ObjectContribution>,
    pub archive_contributions: Vec<ArchiveContribution>,
    pub archive_pulls: Vec<ArchivePullDetail>,
    pub cross_references: Vec<CrossReference>,
    pub memory_regions: Vec<MemoryRegion>,
    pub warnings: Vec<WarningItem>,
}

impl Default for MapIngestResult {
    fn default() -> Self {
        Self {
            detected_toolchain: None,
            resolved_toolchain: ToolchainKind::Gnu,
            linker_family: LinkerFamily::Unknown,
            map_format: MapFormat::Unknown,
            object_contributions: Vec::new(),
            archive_contributions: Vec::new(),
            archive_pulls: Vec::new(),
            cross_references: Vec::new(),
            memory_regions: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

impl MapIngestResult {
    pub fn parser_warnings_count(&self) -> usize {
        self.warnings.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserWarningKind {
    UnknownLine,
    MalformedHex,
    MissingParentSection,
    UnsupportedRowShape,
    HeaderMismatch,
}

trait MapParser {
    fn parse(&self, text: &str) -> Result<MapIngestResult, String>;
}

struct GnuMapParser;
struct LldNativeMapParser;

pub fn parse_map(
    path: &Path,
    selection: ToolchainSelection,
    map_format: MapFormatSelection,
) -> Result<MapIngestResult, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("failed to read map '{}': {err}", path.display()))?;
    parse_map_str(&text, selection, map_format)
}

pub fn parse_map_str(
    text: &str,
    selection: ToolchainSelection,
    map_format: MapFormatSelection,
) -> Result<MapIngestResult, String> {
    let detected_format = detect_map_format(text);
    let detected_toolchain = detect_toolchain_from_format(detected_format);
    let resolved_format = resolve_map_format(map_format, detected_format)?;
    let resolved_toolchain = resolve_toolchain(selection, detected_toolchain, resolved_format)?;
    let parser = parser_for_format(resolved_format);
    let mut result = parser.parse(text)?;
    result.detected_toolchain = detected_toolchain;
    result.resolved_toolchain = resolved_toolchain;
    result.map_format = resolved_format;
    result.linker_family = linker_family_for_toolchain(resolved_toolchain);
    if map_format != MapFormatSelection::Auto && detected_format != MapFormat::Unknown && detected_format != resolved_format {
        result.warnings.push(map_warning(
            "MAP_FORMAT_OVERRIDE",
            ParserWarningKind::HeaderMismatch,
            format!("Requested map format {map_format} overrides detected {detected_format}"),
            None,
        ));
    }
    if selection != ToolchainSelection::Auto && detected_toolchain.is_some() && detected_toolchain != Some(resolved_toolchain) {
        result.warnings.push(map_warning(
            "MAP_TOOLCHAIN_OVERRIDE",
            ParserWarningKind::HeaderMismatch,
            format!("Requested toolchain {selection} overrides detected {}", detected_toolchain.unwrap()),
            None,
        ));
    }
    Ok(result)
}

pub fn detect_map_format(text: &str) -> MapFormat {
    if text
        .lines()
        .any(|line| line.contains("VMA") && line.contains("LMA") && line.contains("Size") && line.contains("Out") && line.contains("In"))
    {
        return MapFormat::LldNative;
    }
    if text.contains("Memory Configuration") || text.lines().any(|line| line.trim_start().starts_with("Linker script and memory map"))
    {
        return MapFormat::Gnu;
    }
    MapFormat::Unknown
}

pub fn detect_toolchain(text: &str) -> Option<ToolchainKind> {
    detect_toolchain_from_format(detect_map_format(text))
}

fn detect_toolchain_from_format(format: MapFormat) -> Option<ToolchainKind> {
    match format {
        MapFormat::Gnu => Some(ToolchainKind::Gnu),
        MapFormat::LldNative => Some(ToolchainKind::Lld),
        MapFormat::Unknown => None,
    }
}

fn resolve_map_format(selection: MapFormatSelection, detected: MapFormat) -> Result<MapFormat, String> {
    match selection {
        MapFormatSelection::Auto => Ok(detected),
        MapFormatSelection::Gnu => {
            if detected == MapFormat::LldNative {
                return Err("map format 'gnu' was forced but the input looks like lld-native; try --map-format lld-native".to_string());
            }
            Ok(MapFormat::Gnu)
        }
        MapFormatSelection::LldNative => {
            if detected == MapFormat::Gnu {
                return Err("map format 'lld-native' was forced but the input looks like GNU ld; try --map-format gnu".to_string());
            }
            Ok(MapFormat::LldNative)
        }
    }
}

fn parser_for_format(format: MapFormat) -> Box<dyn MapParser> {
    match format {
        MapFormat::LldNative => Box::new(LldNativeMapParser),
        MapFormat::Gnu | MapFormat::Unknown => Box::new(GnuMapParser),
    }
}

fn resolve_toolchain(
    selection: ToolchainSelection,
    detected: Option<ToolchainKind>,
    resolved_format: MapFormat,
) -> Result<ToolchainKind, String> {
    match selection {
        ToolchainSelection::Auto => Ok(match resolved_format {
            MapFormat::LldNative => ToolchainKind::Lld,
            MapFormat::Gnu | MapFormat::Unknown => detected.unwrap_or(ToolchainKind::Gnu),
        }),
        ToolchainSelection::Gnu => Ok(ToolchainKind::Gnu),
        ToolchainSelection::Lld => Ok(ToolchainKind::Lld),
        ToolchainSelection::Iar | ToolchainSelection::Armcc | ToolchainSelection::Keil => Err(format!(
            "toolchain '{}' is recognized but not implemented yet; supported values are auto, gnu, and lld",
            selection
        )),
    }
}

fn linker_family_for_toolchain(toolchain: ToolchainKind) -> LinkerFamily {
    match toolchain {
        ToolchainKind::Gnu => LinkerFamily::Gnu,
        ToolchainKind::Lld => LinkerFamily::Lld,
    }
}

impl MapParser for GnuMapParser {
    fn parse(&self, text: &str) -> Result<MapIngestResult, String> {
        Ok(parse_gnu_map_str(text))
    }
}

impl MapParser for LldNativeMapParser {
    fn parse(&self, text: &str) -> Result<MapIngestResult, String> {
        parse_lld_map_str(text)
    }
}

fn parse_gnu_map_str(text: &str) -> MapIngestResult {
    let mut result = MapIngestResult::default();
    result.resolved_toolchain = ToolchainKind::Gnu;
    result.linker_family = LinkerFamily::Gnu;
    result.map_format = MapFormat::Gnu;
    let lines = text.lines().collect::<Vec<_>>();
    let mut index = 0usize;
    let mut current_section: Option<String> = None;
    let mut in_discarded = false;

    while index < lines.len() {
        let line = lines[index].trim_end();
        let trimmed = line.trim();
        if trimmed == "Memory Configuration" {
            in_discarded = false;
            index += 1;
            parse_memory_configuration(&lines, &mut index, &mut result);
            continue;
        }
        if trimmed == "Cross Reference Table" {
            index += 1;
            parse_cross_reference_table(&lines, &mut index, &mut result);
            continue;
        }
        if trimmed.starts_with("Archive member included to satisfy reference by file") {
            index += 1;
            parse_archive_pull_table(&lines, &mut index, &mut result);
            continue;
        }
        if trimmed == "Discarded input sections" {
            in_discarded = true;
            index += 1;
            continue;
        }
        if in_discarded && (trimmed.starts_with("Linker script") || trimmed.starts_with("Memory Configuration")) {
            in_discarded = false;
        }
        if let Some(section_name) = parse_output_section(line) {
            current_section = Some(section_name.to_string());
        } else if !in_discarded && let Some((size, path)) = parse_contribution_line(line) {
            push_contribution(&mut result, current_section.clone(), path.to_string(), size);
        } else if trimmed.starts_with('.') {
            current_section = None;
        } else if trimmed.contains("load address") {
            index += 1;
            continue;
        } else if !trimmed.is_empty()
            && !trimmed.starts_with("Linker script")
            && !trimmed.starts_with("Allocating common symbols")
            && trimmed.chars().next().is_some_and(|c| c.is_alphabetic())
        {
            result.warnings.push(map_warning(
                "MAP_LINE_SKIPPED",
                ParserWarningKind::UnknownLine,
                format!("Skipped unparsed map line {index}: {trimmed}"),
                Some(format!("line:{index}")),
            ));
        }
        index += 1;
    }

    result
}

fn parse_cross_reference_table(lines: &[&str], index: &mut usize, result: &mut MapIngestResult) {
    let mut current_symbol: Option<String> = None;
    let mut current_defined_in: Option<String> = None;
    let mut current_referenced_by = Vec::<String>::new();

    while *index < lines.len() {
        let raw = lines[*index].trim_end();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            if let (Some(symbol), Some(defined_in)) = (current_symbol.take(), current_defined_in.take()) {
                result.cross_references.push(CrossReference {
                    symbol,
                    defined_in,
                    referenced_by: std::mem::take(&mut current_referenced_by),
                });
            }
            *index += 1;
            if *index < lines.len() && !lines[*index].trim().is_empty() {
                continue;
            }
            break;
        }
        if trimmed.starts_with("Symbol") && trimmed.contains("File") {
            *index += 1;
            continue;
        }

        let leading_ws = raw.chars().take_while(|c| c.is_whitespace()).count();
        if leading_ws == 0 {
            if let (Some(symbol), Some(defined_in)) = (current_symbol.take(), current_defined_in.take()) {
                result.cross_references.push(CrossReference {
                    symbol,
                    defined_in,
                    referenced_by: std::mem::take(&mut current_referenced_by),
                });
            }
            if let Some((symbol, file)) = split_symbol_and_file(trimmed) {
                current_symbol = Some(symbol.to_string());
                current_defined_in = Some(file.to_string());
            }
        } else if let Some(file) = trimmed.split_whitespace().next() {
            current_referenced_by.push(file.to_string());
        }
        *index += 1;
    }
    if let (Some(symbol), Some(defined_in)) = (current_symbol.take(), current_defined_in.take()) {
        result.cross_references.push(CrossReference {
            symbol,
            defined_in,
            referenced_by: current_referenced_by,
        });
    }
}

fn parse_archive_pull_table(lines: &[&str], index: &mut usize, result: &mut MapIngestResult) {
    while *index < lines.len() {
        let raw = lines[*index].trim_end();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            *index += 1;
            continue;
        }
        if trimmed.starts_with("Memory Configuration")
            || trimmed.starts_with("Linker script")
            || trimmed.starts_with("Cross Reference Table")
            || trimmed.starts_with("Discarded input sections")
        {
            break;
        }
        if trimmed.eq_ignore_ascii_case("archive member included to satisfy reference by file (symbol)") {
            *index += 1;
            continue;
        }
        if let Some(detail) = parse_archive_pull_row(trimmed) {
            result.archive_pulls.push(detail);
        } else if !trimmed.starts_with("Allocating common symbols") {
            result.warnings.push(map_warning(
                "ARCHIVE_PULL_ROW_SKIPPED",
                ParserWarningKind::UnsupportedRowShape,
                format!("Skipped archive pull row {}: {}", *index, trimmed),
                Some(format!("line:{}", *index)),
            ));
        }
        *index += 1;
    }
}

fn parse_archive_pull_row(line: &str) -> Option<ArchivePullDetail> {
    let split_at = find_column_break(line)?;
    let archive_member = line[..split_at].trim();
    let reference = line[split_at..].trim();
    let open = reference.rfind('(')?;
    let close = reference.rfind(')')?;
    if close <= open {
        return None;
    }
    let referenced_by = reference[..open].trim();
    let symbol = reference[open + 1..close].trim();
    if archive_member.is_empty() || referenced_by.is_empty() || symbol.is_empty() {
        return None;
    }
    Some(ArchivePullDetail {
        archive_member: archive_member.to_string(),
        referenced_by: referenced_by.to_string(),
        symbol: symbol.to_string(),
    })
}

fn find_column_break(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut saw_non_ws = false;
    let mut index = 0usize;
    while index + 1 < bytes.len() {
        let current = bytes[index] as char;
        let next = bytes[index + 1] as char;
        if !current.is_whitespace() {
            saw_non_ws = true;
        }
        if saw_non_ws && current.is_whitespace() && next.is_whitespace() {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn split_symbol_and_file(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.split_whitespace();
    let symbol = parts.next()?;
    let file = parts.next()?;
    Some((symbol, file))
}

fn parse_lld_map_str(text: &str) -> Result<MapIngestResult, String> {
    if detect_map_format(text) != MapFormat::LldNative {
        return Err("input does not match the expected lld-native map header; try --map-format gnu or --map-format auto".to_string());
    }
    let mut result = MapIngestResult::default();
    result.resolved_toolchain = ToolchainKind::Lld;
    result.linker_family = LinkerFamily::Lld;
    result.map_format = MapFormat::LldNative;
    let mut current_section: Option<String> = None;
    let mut parsed_rows = 0usize;
    let mut skipped_rows = 0usize;
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty()
            || (trimmed.contains("VMA") && trimmed.contains("LMA") && trimmed.contains("Size") && trimmed.contains("Out") && trimmed.contains("In"))
        {
            continue;
        }
        if let Some(section_name) = parse_lld_output_section(line) {
            current_section = Some(section_name.to_string());
            parsed_rows += 1;
            continue;
        }
        if is_lld_assignment_row(line) || is_lld_symbol_row(line) {
            continue;
        }
        match parse_lld_contribution_line(line) {
            Ok(Some((size, path))) => {
                parsed_rows += 1;
                if current_section.is_none() {
                    result.warnings.push(map_warning(
                        "LLD_MISSING_PARENT_SECTION",
                        ParserWarningKind::MissingParentSection,
                        format!("lld-native contribution line {index} did not have an output section parent"),
                        Some(format!("line:{index}")),
                    ));
                }
                push_contribution(&mut result, current_section.clone(), path, size);
                continue;
            }
            Ok(None) => {}
            Err(err) => {
                result.warnings.push(map_warning(
                    "LLD_MALFORMED_ROW",
                    ParserWarningKind::MalformedHex,
                    format!("Skipped malformed lld-native row {index}: {err}"),
                    Some(format!("line:{index}")),
                ));
                continue;
            }
        }
        if trimmed.starts_with('.') {
            current_section = None;
            continue;
        }
        if trimmed.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) || trimmed.starts_with('<') {
            skipped_rows += 1;
        }
    }

    if parsed_rows == 0 {
        return Err("lld-native header was detected but no output/input rows could be parsed; the map may be truncated or malformed".to_string());
    }
    if skipped_rows > 0 {
        result.warnings.push(map_warning(
            "MAP_LINE_SKIPPED_SUMMARY",
            ParserWarningKind::UnsupportedRowShape,
            format!("Skipped {skipped_rows} lld-native rows that did not match a supported row shape"),
            None,
        ));
    }
    Ok(result)
}

fn parse_memory_configuration(lines: &[&str], index: &mut usize, result: &mut MapIngestResult) {
    while *index < lines.len() {
        let trimmed = lines[*index].trim();
        if trimmed.is_empty() || trimmed.starts_with("Name") {
            *index += 1;
            continue;
        }
        if trimmed.starts_with("Linker script") || trimmed.starts_with("Memory map") {
            break;
        }
        let parts = trimmed.split_whitespace().collect::<Vec<_>>();
        if parts.len() >= 4 {
            if let (Some(origin), Some(length)) = (parse_num(parts[1]), parse_num(parts[2])) {
                result.memory_regions.push(MemoryRegion {
                    name: parts[0].to_string(),
                    origin,
                    length,
                    attributes: parts[3..].join(" "),
                });
            }
        }
        *index += 1;
    }
}

fn parse_output_section(line: &str) -> Option<&str> {
    if line.starts_with(' ') || line.starts_with('\t') {
        return None;
    }
    let mut parts = line.split_whitespace();
    let name = parts.next()?;
    if !name.starts_with('.') {
        return None;
    }
    let addr = parts.next()?;
    let size = parts.next()?;
    if parse_num(addr).is_some() && parse_num(size).is_some() {
        Some(name)
    } else {
        None
    }
}

fn parse_contribution_line(line: &str) -> Option<(u64, &str)> {
    if !(line.starts_with(' ') || line.starts_with('\t')) {
        return None;
    }
    let parts = line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 3 || !parts[0].starts_with('.') {
        return None;
    }
    let size = parse_num(parts[2])?;
    let path = parts.last()?;
    if path.starts_with("0x") {
        return None;
    }
    Some((size, path))
}

fn parse_lld_output_section(line: &str) -> Option<&str> {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 5 {
        return None;
    }
    if parse_num(parts[0]).is_some() && parse_num(parts[1]).is_some() && parse_num(parts[2]).is_some() && parts[4].starts_with('.') {
        Some(parts[4])
    } else {
        None
    }
}

fn parse_lld_contribution_line(line: &str) -> Result<Option<(u64, String)>, String> {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 5 {
        return Ok(None);
    }
    let Some(vma) = parse_num(parts[0]) else {
        return Ok(None);
    };
    let Some(_lma) = parse_num(parts[1]) else {
        return Ok(None);
    };
    let Some(size) = parse_num(parts[2]) else {
        return Err(format!("failed to parse lld-native size field '{}'", parts[2]));
    };
    let path = parts[4..].join(" ");
    if path.starts_with('.') || path.starts_with("0x") {
        return Ok(None);
    }
    if path != "<internal>" && !path.contains(":(") {
        return Ok(None);
    }
    let path = path
        .split_once(":(")
        .map(|(item, _)| item.to_string())
        .unwrap_or(path);
    if vma == 0 && size == 0 {
        return Ok(Some((size, path)));
    }
    Ok(Some((size, path)))
}

fn is_lld_assignment_row(line: &str) -> bool {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    parts.len() >= 6
        && parse_num(parts[0]).is_some()
        && parse_num(parts[1]).is_some()
        && parse_num(parts[2]).is_some()
        && parts[4] != "<internal>"
        && parts.iter().any(|part| *part == "=")
}

fn is_lld_symbol_row(line: &str) -> bool {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    let tail = if parts.len() > 4 { parts[4..].join(" ") } else { String::new() };
    parts.len() >= 5
        && parse_num(parts[0]).is_some()
        && parse_num(parts[1]).is_some()
        && parse_num(parts[2]).is_some()
        && !parts[4].starts_with('.')
        && !tail.contains(":(")
        && tail != "<internal>"
}

fn push_contribution(result: &mut MapIngestResult, section_name: Option<String>, path: String, size: u64) {
    let source_kind = if path == "<internal>" {
        ObjectSourceKind::Internal
    } else {
        ObjectSourceKind::Object
    };
    result.object_contributions.push(ObjectContribution {
        object_path: path.clone(),
        source_kind,
        section_name: section_name.clone(),
        size,
    });
    if let Some((archive, member)) = split_archive_member(&path) {
        result.archive_contributions.push(ArchiveContribution {
            archive_path: archive.to_string(),
            member_path: Some(member.to_string()),
            section_name,
            size,
        });
    }
}

fn split_archive_member(path: &str) -> Option<(&str, &str)> {
    if let Some(start) = path.find('(') {
        let end = path.rfind(')')?;
        return Some((&path[..start], &path[start + 1..end]));
    }
    let split = path.rsplit_once(':')?;
    if split.0.ends_with(".a") {
        Some(split)
    } else {
        None
    }
}

fn parse_num(text: &str) -> Option<u64> {
    if let Some(hex) = text.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()
    } else if let Some(value) = parse_scaled_num(text) {
        Some(value)
    } else if text.chars().any(|ch| ch.is_ascii_hexdigit() && ch.is_ascii_alphabetic()) {
        u64::from_str_radix(text, 16).ok()
    } else {
        text.parse().ok()
    }
}

fn parse_scaled_num(text: &str) -> Option<u64> {
    let (number, scale) = match text.chars().last()? {
        'K' | 'k' => (&text[..text.len() - 1], 1024u64),
        'M' | 'm' => (&text[..text.len() - 1], 1024u64 * 1024),
        'G' | 'g' => (&text[..text.len() - 1], 1024u64 * 1024 * 1024),
        _ => return None,
    };
    parse_num(number).map(|value| value.saturating_mul(scale))
}

fn map_warning(code: &str, kind: ParserWarningKind, message: String, related: Option<String>) -> WarningItem {
    WarningItem {
        level: WarningLevel::Info,
        code: code.to_string(),
        message: format!("[{kind:?}] {message}"),
        source: WarningSource::Map,
        related,
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_map_format, detect_toolchain, parse_map_str, parse_num};
    use crate::model::{MapFormat, MapFormatSelection, ObjectSourceKind, ToolchainKind, ToolchainSelection};

    #[test]
    fn parses_gnu_ld_map_snippet() {
        let text = include_str!("../../../tests/fixtures/sample.map");
        let result = parse_map_str(text, ToolchainSelection::Auto, MapFormatSelection::Auto).unwrap();
        assert_eq!(result.memory_regions.len(), 2);
        assert!(result.object_contributions.iter().any(|item| item.object_path.ends_with("main.o")));
        assert!(result.archive_contributions.iter().any(|item| item.archive_path.ends_with("libapp.a")));
        assert_eq!(result.resolved_toolchain, ToolchainKind::Gnu);
    }

    #[test]
    fn tolerates_broken_lines() {
        let text = include_str!("../../../tests/fixtures/broken.map");
        let result = parse_map_str(text, ToolchainSelection::Auto, MapFormatSelection::Auto).unwrap();
        assert!(!result.warnings.is_empty());
        assert!(!result.object_contributions.is_empty());
    }

    #[test]
    fn parses_archive_member_colon_style() {
        let text = include_str!("../../../tests/fixtures/archive_colon.map");
        let result = parse_map_str(text, ToolchainSelection::Auto, MapFormatSelection::Auto).unwrap();
        assert!(result
            .archive_contributions
            .iter()
            .any(|item| item.archive_path.ends_with("libcolon.a") && item.member_path.as_deref() == Some("member.o")));
    }

    #[test]
    fn parses_cross_reference_table() {
        let text = include_str!("../../../tests/fixtures/cross_reference.map");
        let result = parse_map_str(text, ToolchainSelection::Auto, MapFormatSelection::Auto).unwrap();
        assert!(result
            .cross_references
            .iter()
            .any(|item| item.symbol == "startup_entry" && item.defined_in.ends_with("libapp.a(startup.o)") && item.referenced_by == vec!["build/main.o"]));
    }

    #[test]
    fn parses_archive_pull_table() {
        let text = include_str!("../../../tests/fixtures/archive_pull.map");
        let result = parse_map_str(text, ToolchainSelection::Auto, MapFormatSelection::Auto).unwrap();
        assert!(result.archive_pulls.iter().any(|item| {
            item.archive_member == "libapp.a(startup.o)"
                && item.referenced_by == "build/main.o"
                && item.symbol == "startup_entry"
        }));
    }

    #[test]
    fn tolerates_map_without_memory_configuration() {
        let text = include_str!("../../../tests/fixtures/no_memory_config.map");
        let result = parse_map_str(text, ToolchainSelection::Auto, MapFormatSelection::Auto).unwrap();
        assert!(result.memory_regions.is_empty());
        assert!(!result.object_contributions.is_empty());
    }

    #[test]
    fn parses_decimal_sizes_and_tab_indentation() {
        let decimal = parse_map_str(
            include_str!("../../../tests/fixtures/decimal_sizes.map"),
            ToolchainSelection::Auto,
            MapFormatSelection::Auto,
        )
        .unwrap();
        assert_eq!(decimal.object_contributions[0].size, 32);

        let tabbed = parse_map_str(
            include_str!("../../../tests/fixtures/tab_indented.map"),
            ToolchainSelection::Auto,
            MapFormatSelection::Auto,
        )
        .unwrap();
        assert!(tabbed.object_contributions.iter().any(|item| item.object_path.ends_with("tabbed.o")));
    }

    #[test]
    fn ignores_known_non_contribution_blocks() {
        let result = parse_map_str(
            include_str!("../../../tests/fixtures/unparsed_block.map"),
            ToolchainSelection::Auto,
            MapFormatSelection::Auto,
        )
        .unwrap();
        assert!(result.warnings.iter().all(|warning| warning.code != "MAP_LINE_SKIPPED"));
        assert!(result.object_contributions.iter().any(|item| item.object_path.ends_with("common.o")));
    }

    #[test]
    fn keeps_loading_when_output_section_has_load_address() {
        let result = parse_map_str(
            include_str!("../../../tests/fixtures/load_address.map"),
            ToolchainSelection::Auto,
            MapFormatSelection::Auto,
        )
        .unwrap();
        assert!(result.object_contributions.iter().any(|item| item.object_path.ends_with("load.o")));
    }

    #[test]
    fn ignores_discarded_sections_block() {
        let result = parse_map_str(
            include_str!("../../../tests/fixtures/discarded_sections.map"),
            ToolchainSelection::Auto,
            MapFormatSelection::Auto,
        )
        .unwrap();
        assert!(result.object_contributions.iter().any(|item| item.object_path.ends_with("main.o")));
        assert!(!result.object_contributions.iter().any(|item| item.object_path.ends_with("unused.o")));
    }

    #[test]
    fn preserves_non_ascii_object_paths() {
        let result = parse_map_str(
            include_str!("../../../tests/fixtures/non_ascii.map"),
            ToolchainSelection::Auto,
            MapFormatSelection::Auto,
        )
        .unwrap();
        assert!(result.object_contributions.iter().any(|item| item.object_path.contains("naïve_utf8.o")));
        assert!(result.object_contributions.iter().any(|item| item.object_path.contains("cpp_長名.o")));
    }

    #[test]
    fn detects_and_parses_lld_map() {
        let text = include_str!("../../../tests/fixtures/sample_lld.map");
        assert_eq!(detect_map_format(text), MapFormat::LldNative);
        assert_eq!(detect_toolchain(text), Some(ToolchainKind::Lld));
        let result = parse_map_str(text, ToolchainSelection::Auto, MapFormatSelection::Auto).unwrap();
        assert_eq!(result.resolved_toolchain, ToolchainKind::Lld);
        assert!(result.object_contributions.iter().any(|item| item.object_path.ends_with("main.o")));
        assert!(result
            .archive_contributions
            .iter()
            .any(|item| item.archive_path.ends_with("libutil.a") && item.member_path.as_deref() == Some("util.o")));
    }

    #[test]
    fn parses_lld_cpp_and_internal_entries() {
        let cpp = parse_map_str(
            include_str!("../../../tests/fixtures/lld_cpp.map"),
            ToolchainSelection::Auto,
            MapFormatSelection::Auto,
        )
        .unwrap();
        assert!(cpp.object_contributions.iter().any(|item| item.object_path.ends_with("app/main.o")));
        assert!(cpp
            .archive_contributions
            .iter()
            .any(|item| item.archive_path.ends_with("libcore.a") && item.member_path.as_deref() == Some("core.o")));

        let internal = parse_map_str(
            include_str!("../../../tests/fixtures/lld_internal.map"),
            ToolchainSelection::Auto,
            MapFormatSelection::Auto,
        )
        .unwrap();
        assert!(internal
            .object_contributions
            .iter()
            .any(|item| item.object_path == "<internal>" && item.source_kind == ObjectSourceKind::Internal));
        assert!(!internal.warnings.iter().any(|item| item.code == "LLD_INTERNAL_ENTRY_SUMMARY"));
    }

    #[test]
    fn preserves_lld_paths_with_spaces_and_warns_on_malformed_rows() {
        let paths = parse_map_str(
            include_str!("../../../tests/fixtures/lld_path_spaces.map"),
            ToolchainSelection::Auto,
            MapFormatSelection::Auto,
        )
        .unwrap();
        assert!(paths
            .object_contributions
            .iter()
            .any(|item| item.object_path.contains("Program Files/fw build/data object.o")));
        assert!(paths
            .archive_contributions
            .iter()
            .any(|item| item.archive_path.contains("with spaces/libutil.a")));

        let malformed = parse_map_str(
            include_str!("../../../tests/fixtures/lld_malformed_hex.map"),
            ToolchainSelection::Auto,
            MapFormatSelection::Auto,
        )
        .unwrap();
        assert!(malformed.object_contributions.iter().any(|item| item.object_path.ends_with("valid.o")));
        assert!(malformed.warnings.iter().any(|item| item.code == "LLD_MALFORMED_ROW"));
    }

    #[test]
    fn ignores_common_lld_symbol_and_assignment_rows_without_warning_spam() {
        let demo = include_str!("../../../tests/fixtures/lld_internal.map");
        let result = parse_map_str(demo, ToolchainSelection::Auto, MapFormatSelection::Auto).unwrap();
        assert!(result.object_contributions.iter().any(|item| item.object_path.contains("<internal>")));
        assert_eq!(result.warnings.iter().filter(|item| item.code == "MAP_LINE_SKIPPED").count(), 0);
        assert!(result.warnings.iter().filter(|item| item.code == "LLD_MALFORMED_ROW").count() <= 5);
    }

    #[test]
    fn parses_bare_hex_and_scaled_lld_numbers() {
        assert_eq!(parse_num("f8000000"), Some(0xf8000000));
        assert_eq!(parse_num("5af78"), Some(0x5af78));
        assert_eq!(parse_num("64K"), Some(64 * 1024));
    }

    #[test]
    fn detects_unknown_map_format() {
        assert_eq!(detect_map_format("random text"), MapFormat::Unknown);
    }

    #[test]
    fn rejects_unimplemented_toolchain_family() {
        let text = include_str!("../../../tests/fixtures/sample.map");
        let err = parse_map_str(text, ToolchainSelection::Iar, MapFormatSelection::Auto).unwrap_err();
        assert!(err.contains("not implemented"));
    }

    #[test]
    fn map_format_rejects_obvious_mismatch() {
        let text = include_str!("../../../tests/fixtures/sample.map");
        let err = parse_map_str(text, ToolchainSelection::Auto, MapFormatSelection::LldNative).unwrap_err();
        assert!(err.contains("looks like GNU ld"));
    }
}
