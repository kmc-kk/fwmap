use std::fs;
use std::path::Path;

use crate::model::{ArchiveContribution, MemoryRegion, ObjectContribution};

#[derive(Debug, Clone, Default)]
pub struct MapIngestResult {
    pub object_contributions: Vec<ObjectContribution>,
    pub archive_contributions: Vec<ArchiveContribution>,
    pub memory_regions: Vec<MemoryRegion>,
    pub warnings: Vec<String>,
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

    while index < lines.len() {
        let line = lines[index].trim_end();
        let trimmed = line.trim();
        if trimmed == "Memory Configuration" {
            index += 1;
            parse_memory_configuration(&lines, &mut index, &mut result);
            continue;
        }
        if let Some(section_name) = parse_output_section(line) {
            current_section = Some(section_name.to_string());
        } else if let Some((size, path)) = parse_contribution_line(line) {
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
            result.warnings.push(format!("skipped unparsed map line: {trimmed}"));
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
    let start = path.find('(')?;
    let end = path.rfind(')')?;
    Some((&path[..start], &path[start + 1..end]))
}

fn parse_num(text: &str) -> Option<u64> {
    if let Some(hex) = text.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()
    } else {
        text.parse().ok()
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
}
