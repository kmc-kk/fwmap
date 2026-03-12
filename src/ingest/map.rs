use std::fs;
use std::path::Path;

use crate::model::{ArchiveContribution, MemoryRegion, ObjectContribution, WarningItem, WarningLevel, WarningSource};

#[derive(Debug, Clone, Default)]
pub struct MapIngestResult {
    pub object_contributions: Vec<ObjectContribution>,
    pub archive_contributions: Vec<ArchiveContribution>,
    pub memory_regions: Vec<MemoryRegion>,
    pub warnings: Vec<WarningItem>,
}

pub fn parse_map(path: &Path) -> Result<MapIngestResult, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("failed to read map '{}': {err}", path.display()))?;
    Ok(parse_map_str(&text))
}

pub fn parse_map_str(text: &str) -> MapIngestResult {
    let mut result = MapIngestResult::default();
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
            let section_name = current_section.clone();
            result.object_contributions.push(ObjectContribution {
                object_path: path.to_string(),
                section_name: section_name.clone(),
                size,
            });
            if let Some((archive, member)) = split_archive_member(path) {
                result.archive_contributions.push(ArchiveContribution {
                    archive_path: archive.to_string(),
                    member_path: Some(member.to_string()),
                    section_name,
                    size,
                });
            }
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
                format!("Skipped unparsed map line {index}: {trimmed}"),
                Some(format!("line:{index}")),
            ));
        }
        index += 1;
    }

    result
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
    } else {
        text.parse().ok()
    }
}

fn map_warning(code: &str, message: String, related: Option<String>) -> WarningItem {
    WarningItem {
        level: WarningLevel::Info,
        code: code.to_string(),
        message,
        source: WarningSource::Map,
        related,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_map_str;

    #[test]
    fn parses_gnu_ld_map_snippet() {
        let text = include_str!("../../tests/fixtures/sample.map");
        let result = parse_map_str(text);
        assert_eq!(result.memory_regions.len(), 2);
        assert!(result.object_contributions.iter().any(|item| item.object_path.ends_with("main.o")));
        assert!(result.archive_contributions.iter().any(|item| item.archive_path.ends_with("libapp.a")));
    }

    #[test]
    fn tolerates_broken_lines() {
        let text = include_str!("../../tests/fixtures/broken.map");
        let result = parse_map_str(text);
        assert!(!result.warnings.is_empty());
        assert!(!result.object_contributions.is_empty());
    }

    #[test]
    fn parses_archive_member_colon_style() {
        let text = include_str!("../../tests/fixtures/archive_colon.map");
        let result = parse_map_str(text);
        assert!(result
            .archive_contributions
            .iter()
            .any(|item| item.archive_path.ends_with("libcolon.a") && item.member_path.as_deref() == Some("member.o")));
    }

    #[test]
    fn tolerates_map_without_memory_configuration() {
        let text = include_str!("../../tests/fixtures/no_memory_config.map");
        let result = parse_map_str(text);
        assert!(result.memory_regions.is_empty());
        assert!(!result.object_contributions.is_empty());
    }

    #[test]
    fn parses_decimal_sizes_and_tab_indentation() {
        let decimal = parse_map_str(include_str!("../../tests/fixtures/decimal_sizes.map"));
        assert_eq!(decimal.object_contributions[0].size, 32);

        let tabbed = parse_map_str(include_str!("../../tests/fixtures/tab_indented.map"));
        assert!(tabbed.object_contributions.iter().any(|item| item.object_path.ends_with("tabbed.o")));
    }

    #[test]
    fn ignores_known_non_contribution_blocks() {
        let result = parse_map_str(include_str!("../../tests/fixtures/unparsed_block.map"));
        assert!(result.warnings.iter().all(|warning| warning.code != "MAP_LINE_SKIPPED"));
        assert!(result.object_contributions.iter().any(|item| item.object_path.ends_with("common.o")));
    }

    #[test]
    fn keeps_loading_when_output_section_has_load_address() {
        let result = parse_map_str(include_str!("../../tests/fixtures/load_address.map"));
        assert!(result.object_contributions.iter().any(|item| item.object_path.ends_with("load.o")));
    }

    #[test]
    fn ignores_discarded_sections_block() {
        let result = parse_map_str(include_str!("../../tests/fixtures/discarded_sections.map"));
        assert!(result.object_contributions.iter().any(|item| item.object_path.ends_with("main.o")));
        assert!(!result.object_contributions.iter().any(|item| item.object_path.ends_with("unused.o")));
    }

    #[test]
    fn preserves_non_ascii_object_paths() {
        let result = parse_map_str(include_str!("../../tests/fixtures/non_ascii.map"));
        assert!(result.object_contributions.iter().any(|item| item.object_path.contains("naïve_utf8.o")));
        assert!(result.object_contributions.iter().any(|item| item.object_path.contains("cpp_長名.o")));
    }
}
