use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::model::{LinkerScriptInfo, MemoryRegion, SectionPlacement, WarningItem, WarningLevel, WarningSource};

#[derive(Debug, Clone)]
pub struct LdsIngestResult {
    pub linker_script: LinkerScriptInfo,
    pub warnings: Vec<WarningItem>,
}

pub fn parse_lds(path: &Path) -> Result<LdsIngestResult, String> {
    let mut warnings = Vec::new();
    let mut visited = std::collections::BTreeSet::new();
    let text = load_lds_with_includes(path, &mut visited, &mut warnings)?;
    let mut result = parse_lds_str(path.display().to_string(), &text);
    warnings.append(&mut result.warnings);
    result.warnings = warnings;
    Ok(result)
}

pub fn parse_lds_str(path: String, text: &str) -> LdsIngestResult {
    let mut warnings = Vec::new();
    let lines = text.lines().collect::<Vec<_>>();
    let mut regions = Vec::new();
    let mut placements = Vec::new();
    let mut in_memory = false;
    let mut in_sections = false;
    let mut brace_depth = 0i32;
    let mut current_section: Option<SectionPlacement> = None;

    for (idx, raw_line) in lines.iter().enumerate() {
        let line = strip_comments(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("MEMORY") {
            in_memory = true;
            brace_depth += line.matches('{').count() as i32;
            brace_depth -= line.matches('}').count() as i32;
            continue;
        }
        if line.starts_with("SECTIONS") {
            in_sections = true;
            brace_depth += line.matches('{').count() as i32;
            brace_depth -= line.matches('}').count() as i32;
            continue;
        }

        if in_memory {
            if let Some(region) = parse_memory_line(line) {
                regions.push(region);
            }
        } else if in_sections {
            if let Some(section) = start_section_block(line) {
                if line.contains('}') && !section.region_name.is_empty() {
                    placements.push(section);
                } else {
                    current_section = Some(section);
                }
            } else if let Some(existing) = current_section.as_mut() {
                if existing.align.is_none() {
                    existing.align = extract_number_after(line, "ALIGN");
                }
                if line.contains("KEEP(") {
                    existing.keep = true;
                }
                if line.contains("AT") {
                    existing.has_at = true;
                    if existing.load_region_name.is_none() {
                        existing.load_region_name = extract_named_after(line, "AT", '>');
                    }
                }
                if existing.region_name.is_empty() {
                    if let Some(region_name) = extract_after_marker(line, '>') {
                        existing.region_name = region_name;
                    }
                }
                if line.contains('}') && !existing.region_name.is_empty() {
                    placements.push(existing.clone());
                    current_section = None;
                }
            } else if line.starts_with('.') {
                warnings.push(WarningItem {
                    level: WarningLevel::Info,
                    code: "LDS_SECTION_SKIPPED".to_string(),
                    message: format!("Skipped linker script section line {}: {}", idx + 1, line),
                    source: WarningSource::Analyze,
                    related: Some(format!("line:{}", idx + 1)),
                });
            }
        }

        brace_depth += line.matches('{').count() as i32;
        brace_depth -= line.matches('}').count() as i32;
        if brace_depth <= 0 {
            in_memory = false;
            in_sections = false;
            brace_depth = 0;
        }
    }

    if regions.is_empty() {
        regions = derive_solid_memory_regions(text);
    }

    LdsIngestResult {
        linker_script: LinkerScriptInfo {
            path,
            regions,
            placements,
        },
        warnings,
    }
}

fn strip_comments(line: &str) -> &str {
    line.split("/*").next().unwrap_or(line).split("//").next().unwrap_or(line)
}

fn load_lds_with_includes(
    path: &Path,
    visited: &mut std::collections::BTreeSet<PathBuf>,
    warnings: &mut Vec<WarningItem>,
) -> Result<String, String> {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical.clone()) {
        warnings.push(WarningItem {
            level: WarningLevel::Info,
            code: "LDS_INCLUDE_SKIPPED".to_string(),
            message: format!("Skipped already included linker script '{}'", canonical.display()),
            source: WarningSource::Analyze,
            related: Some(canonical.display().to_string()),
        });
        return Ok(String::new());
    }

    let text = fs::read_to_string(path).map_err(|err| format!("failed to read linker script '{}': {err}", path.display()))?;
    let mut merged = String::new();
    for raw_line in text.lines() {
        let line = strip_comments(raw_line).trim();
        if let Some(include_path) = parse_include_line(line) {
            let resolved = path.parent().unwrap_or_else(|| Path::new(".")).join(include_path);
            if resolved.exists() {
                merged.push_str(&load_lds_with_includes(&resolved, visited, warnings)?);
                merged.push('\n');
            } else {
                warnings.push(WarningItem {
                    level: WarningLevel::Warn,
                    code: "LDS_INCLUDE_NOT_FOUND".to_string(),
                    message: format!(
                        "Included linker script '{}' was not found while reading '{}'",
                        resolved.display(),
                        path.display()
                    ),
                    source: WarningSource::Analyze,
                    related: Some(resolved.display().to_string()),
                });
            }
            continue;
        }
        merged.push_str(raw_line);
        merged.push('\n');
    }
    Ok(merged)
}

fn parse_include_line(line: &str) -> Option<String> {
    let rest = line.strip_prefix("INCLUDE")?.trim();
    if rest.is_empty() {
        return None;
    }
    Some(rest.trim_matches('"').to_string())
}

fn parse_memory_line(line: &str) -> Option<MemoryRegion> {
    let normalized = line.replace(',', " ");
    let parts = normalized.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 6 || parts[0] == "{" || parts[0] == "}" {
        return None;
    }
    let origin_idx = parts.iter().position(|item| item.eq_ignore_ascii_case("ORIGIN"))?;
    let length_idx = parts.iter().position(|item| item.eq_ignore_ascii_case("LENGTH"))?;
    let origin = parse_assignment_value(parts.get(origin_idx + 2).copied()?)?;
    let length = parse_assignment_value(parts.get(length_idx + 2).copied()?)?;
    Some(MemoryRegion {
        name: parts[0].to_string(),
        origin,
        length,
        attributes: parts.get(1).copied().unwrap_or_default().trim_matches(|c| c == '(' || c == ')').to_string(),
    })
}

fn start_section_block(line: &str) -> Option<SectionPlacement> {
    if !line.starts_with('.') {
        return None;
    }
    let section_name = line.split_whitespace().next()?.trim_end_matches(':').to_string();
    Some(SectionPlacement {
        section_name,
        region_name: extract_after_marker(line, '>').unwrap_or_default(),
        load_region_name: extract_named_after(line, "AT", '>'),
        align: extract_number_after(line, "ALIGN"),
        keep: line.contains("KEEP("),
        has_at: line.contains("AT"),
    })
}

fn extract_after_marker(line: &str, marker: char) -> Option<String> {
    let idx = line.rfind(marker)?;
    let tail = &line[idx + 1..];
    Some(
        tail.split(|c: char| c.is_whitespace() || c == '{' || c == '}')
            .find(|token| !token.is_empty())?
            .to_string(),
    )
}

fn extract_named_after(line: &str, keyword: &str, marker: char) -> Option<String> {
    let idx = line.find(keyword)?;
    extract_after_marker(&line[idx..], marker)
}

fn extract_number_after(line: &str, keyword: &str) -> Option<u64> {
    let idx = line.find(keyword)?;
    let tail = &line[idx + keyword.len()..];
    let start = tail.find(|c: char| c.is_ascii_digit())?;
    let num = &tail[start..].split(|c: char| c == ')' || c == ',' || c.is_whitespace()).next()?;
    parse_assignment_value(num)
}

fn parse_assignment_value(value: &str) -> Option<u64> {
    let trimmed = value.trim().trim_end_matches(',');
    if let Some(hex) = trimmed.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()
    } else if let Some(k) = trimmed.strip_suffix('K').or_else(|| trimmed.strip_suffix('k')) {
        k.parse::<u64>().ok().map(|v| v * 1024)
    } else if let Some(m) = trimmed.strip_suffix('M').or_else(|| trimmed.strip_suffix('m')) {
        m.parse::<u64>().ok().map(|v| v * 1024 * 1024)
    } else {
        trimmed.parse::<u64>().ok()
    }
}

fn derive_solid_memory_regions(text: &str) -> Vec<MemoryRegion> {
    let mut virtual_addresses = std::collections::BTreeMap::<String, u64>::new();
    let mut physical_addresses = std::collections::BTreeMap::<String, u64>::new();
    let mut sizes = std::collections::BTreeMap::<String, u64>::new();

    for raw_line in text.lines() {
        let line = strip_comments(raw_line).trim();
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let symbol = name.trim();
        let parsed = parse_assignment_value(value.trim().trim_end_matches(';'));
        if let Some(base) = symbol.strip_suffix("_VirtualAddress") {
            if let Some(value) = parsed {
                virtual_addresses.insert(base.to_string(), value);
            }
        } else if let Some(base) = symbol.strip_suffix("_PhysicalAddress") {
            if let Some(value) = parsed {
                physical_addresses.insert(base.to_string(), value);
            }
        } else if let Some(base) = symbol.strip_suffix("_Size") {
            if let Some(value) = parsed {
                sizes.insert(base.to_string(), value);
            }
        }
    }

    let mut regions = sizes
        .into_iter()
        .filter_map(|(base, length)| {
            let origin = virtual_addresses
                .get(&base)
                .copied()
                .or_else(|| physical_addresses.get(&base).copied())?;
            Some(MemoryRegion {
                name: base.trim_start_matches("_smm_").to_string(),
                origin,
                length,
                attributes: String::new(),
            })
        })
        .collect::<Vec<_>>();
    regions.sort_by(|a, b| a.origin.cmp(&b.origin).then_with(|| a.name.cmp(&b.name)));
    regions
}

#[cfg(test)]
mod tests {
    use super::{parse_lds, parse_lds_str};
    use std::path::Path;

    #[test]
    fn parses_memory_and_sections_subset() {
        let text = include_str!("../../../tests/fixtures/sample.ld");
        let result = parse_lds_str("sample.ld".to_string(), text);
        assert_eq!(result.linker_script.regions.len(), 2);
        assert!(result.linker_script.placements.iter().any(|p| p.section_name == ".text" && p.region_name == "FLASH"));
        assert!(result.linker_script.placements.iter().any(|p| p.load_region_name.as_deref() == Some("FLASH")));
    }

    #[test]
    fn resolves_included_memory_regions() {
        let result = parse_lds(Path::new("tests/fixtures/include_main.ld")).unwrap();
        assert!(result.linker_script.regions.iter().any(|item| item.name == "FLASH"));
        assert!(result.linker_script.placements.iter().any(|item| item.section_name == ".text"));
    }

    #[test]
    fn derives_solid_memory_map_regions_from_assignments() {
        let text = include_str!("../../../tests/fixtures/solid_memory_map.ld");
        let result = parse_lds_str("solid_cs.ld".to_string(), text);
        assert!(result.linker_script.regions.iter().any(|item| item.name == "SOLID"));
        assert!(result
            .linker_script
            .regions
            .iter()
            .any(|item| item.name == "DATARAM" && item.origin == 0x2000_0000));
    }
}
