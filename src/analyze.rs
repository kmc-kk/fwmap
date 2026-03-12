use std::collections::BTreeMap;
use std::path::Path;

use crate::ingest::{elf, map};
use crate::model::{
    AnalysisResult, ArchiveContribution, DiffResult, MemorySummary, ObjectContribution, SectionCategory, SectionInfo,
    SectionTotal, SymbolInfo, WarningItem, WarningLevel, WarningSource,
};

const ROM_USAGE_THRESHOLD: f64 = 0.85;
const RAM_USAGE_THRESHOLD: f64 = 0.85;
const LARGE_SYMBOL_THRESHOLD: u64 = 4 * 1024;
const GROWTH_RATE_THRESHOLD: f64 = 0.05;

pub fn analyze_paths(elf_path: &Path, map_path: Option<&Path>) -> Result<AnalysisResult, String> {
    let elf = elf::parse_elf(elf_path)?;
    let map_data = match map_path {
        Some(path) => Some(map::parse_map(path)?),
        None => None,
    };
    let memory = build_memory_summary(&elf.sections, map_data.as_ref().map(|item| item.memory_regions.as_slice()).unwrap_or(&[]));
    let mut warnings = elf.warnings;
    if let Some(map_data) = map_data.as_ref() {
        warnings.extend(map_data.warnings.clone());
    }

    let mut result = AnalysisResult {
        binary: elf.binary,
        sections: elf.sections,
        symbols: sorted_symbols(elf.symbols),
        object_contributions: aggregate_objects(map_data.as_ref().map(|item| item.object_contributions.as_slice()).unwrap_or(&[])),
        archive_contributions: aggregate_archives(map_data.as_ref().map(|item| item.archive_contributions.as_slice()).unwrap_or(&[])),
        memory,
        warnings,
    };
    result.warnings.extend(evaluate_warnings(&result, None));
    Ok(result)
}

pub fn build_memory_summary(sections: &[SectionInfo], regions: &[crate::model::MemoryRegion]) -> MemorySummary {
    let mut rom_bytes = 0u64;
    let mut ram_bytes = 0u64;
    let mut totals = sections
        .iter()
        .map(|section| {
            match section.category {
                SectionCategory::Rom => rom_bytes += section.size,
                SectionCategory::Ram => ram_bytes += section.size,
                SectionCategory::Other => {}
            }
            SectionTotal {
                section_name: section.name.clone(),
                size: section.size,
                category: section.category,
            }
        })
        .collect::<Vec<_>>();
    totals.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.section_name.cmp(&b.section_name)));

    MemorySummary {
        rom_bytes,
        ram_bytes,
        section_totals: totals,
        memory_regions: regions.to_vec(),
    }
}

pub fn sorted_symbols(mut symbols: Vec<SymbolInfo>) -> Vec<SymbolInfo> {
    symbols.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    symbols
}

pub fn evaluate_warnings(current: &AnalysisResult, diff: Option<&DiffResult>) -> Vec<WarningItem> {
    let mut warnings = Vec::new();
    let rom_capacity = memory_capacity(&current.memory.memory_regions, &["rom", "flash"]);
    if let Some(capacity) = rom_capacity {
        let ratio = current.memory.rom_bytes as f64 / capacity as f64;
        if ratio >= ROM_USAGE_THRESHOLD {
            warnings.push(WarningItem {
                level: WarningLevel::Warn,
                code: "ROM_THRESHOLD".to_string(),
                message: format!("ROM usage exceeded {:.0}% ({:.1}%)", ROM_USAGE_THRESHOLD * 100.0, ratio * 100.0),
                source: WarningSource::Analyze,
                related: Some("rom".to_string()),
            });
        }
    }
    let ram_capacity = memory_capacity(&current.memory.memory_regions, &["ram"]);
    if let Some(capacity) = ram_capacity {
        let ratio = current.memory.ram_bytes as f64 / capacity as f64;
        if ratio >= RAM_USAGE_THRESHOLD {
            warnings.push(WarningItem {
                level: WarningLevel::Warn,
                code: "RAM_THRESHOLD".to_string(),
                message: format!("RAM usage exceeded {:.0}% ({:.1}%)", RAM_USAGE_THRESHOLD * 100.0, ratio * 100.0),
                source: WarningSource::Analyze,
                related: Some("ram".to_string()),
            });
        }
    }

    for symbol in current.symbols.iter().filter(|item| item.size >= LARGE_SYMBOL_THRESHOLD).take(5) {
        warnings.push(WarningItem {
            level: WarningLevel::Warn,
            code: "LARGE_SYMBOL".to_string(),
            message: format!("Large symbol detected: {} ({})", symbol.name, format_bytes(symbol.size)),
            source: WarningSource::Analyze,
            related: Some(symbol.name.clone()),
        });
    }

    if let Some(diff) = diff {
        for name in [".data", ".bss"] {
            if let Some(entry) = diff.section_diffs.iter().find(|entry| entry.name == name && entry.previous > 0) {
                let growth = entry.delta as f64 / entry.previous as f64;
                if growth >= GROWTH_RATE_THRESHOLD {
                    warnings.push(WarningItem {
                        level: WarningLevel::Warn,
                        code: format!("{}_GROWTH", name.trim_start_matches('.').to_uppercase()),
                        message: format!("{name} grew by {:.1}% ({:+})", growth * 100.0, entry.delta),
                        source: WarningSource::Analyze,
                        related: Some(name.to_string()),
                    });
                }
            }
        }
        if let Some(entry) = diff.symbol_diffs.iter().find(|entry| entry.delta >= LARGE_SYMBOL_THRESHOLD as i64) {
            warnings.push(WarningItem {
                level: WarningLevel::Warn,
                code: "SYMBOL_SPIKE".to_string(),
                message: format!("Symbol growth spike: {} ({:+})", entry.name, entry.delta),
                source: WarningSource::Analyze,
                related: Some(entry.name.clone()),
            });
        }
    }

    warnings
}

pub fn format_bytes(bytes: u64) -> String {
    format!("{bytes} bytes ({:.2} KiB)", bytes as f64 / 1024.0)
}

fn memory_capacity(regions: &[crate::model::MemoryRegion], names: &[&str]) -> Option<u64> {
    let total = regions
        .iter()
        .filter(|region| names.iter().any(|name| region.name.eq_ignore_ascii_case(name)))
        .map(|region| region.length)
        .sum::<u64>();
    (total > 0).then_some(total)
}

fn aggregate_objects(items: &[ObjectContribution]) -> Vec<ObjectContribution> {
    let mut totals = BTreeMap::<(String, Option<String>), u64>::new();
    for item in items {
        *totals.entry((item.object_path.clone(), item.section_name.clone())).or_default() += item.size;
    }
    let mut result = totals
        .into_iter()
        .map(|((object_path, section_name), size)| ObjectContribution {
            object_path,
            section_name,
            size,
        })
        .collect::<Vec<_>>();
    result.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.object_path.cmp(&b.object_path)));
    result
}

fn aggregate_archives(items: &[ArchiveContribution]) -> Vec<ArchiveContribution> {
    let mut totals = BTreeMap::<(String, Option<String>, Option<String>), u64>::new();
    for item in items {
        *totals
            .entry((item.archive_path.clone(), item.member_path.clone(), item.section_name.clone()))
            .or_default() += item.size;
    }
    let mut result = totals
        .into_iter()
        .map(|((archive_path, member_path, section_name), size)| ArchiveContribution {
            archive_path,
            member_path,
            section_name,
            size,
        })
        .collect::<Vec<_>>();
    result.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.archive_path.cmp(&b.archive_path)));
    result
}

#[cfg(test)]
mod tests {
    use super::{build_memory_summary, evaluate_warnings, sorted_symbols};
    use crate::diff::diff_results;
    use crate::model::{
        AnalysisResult, BinaryInfo, DiffChangeKind, DiffResult, DiffSummary, MemoryRegion, MemorySummary,
        SectionCategory, SectionInfo, SectionTotal, SymbolInfo,
    };

    #[test]
    fn classifies_rom_and_ram_totals() {
        let sections = vec![
            SectionInfo {
                name: ".text".to_string(),
                addr: 0,
                size: 100,
                flags: vec!["ALLOC".to_string(), "EXEC".to_string()],
                category: SectionCategory::Rom,
            },
            SectionInfo {
                name: ".data".to_string(),
                addr: 0,
                size: 32,
                flags: vec!["ALLOC".to_string(), "WRITE".to_string()],
                category: SectionCategory::Ram,
            },
        ];
        let summary = build_memory_summary(&sections, &[]);
        assert_eq!(summary.rom_bytes, 100);
        assert_eq!(summary.ram_bytes, 32);
    }

    #[test]
    fn sorts_symbols_by_size() {
        let symbols = vec![
            SymbolInfo {
                name: "small".to_string(),
                demangled_name: None,
                section_name: None,
                object_path: None,
                size: 1,
            },
            SymbolInfo {
                name: "big".to_string(),
                demangled_name: None,
                section_name: None,
                object_path: None,
                size: 10,
            },
        ];
        let sorted = sorted_symbols(symbols);
        assert_eq!(sorted[0].name, "big");
    }

    #[test]
    fn computes_diffs_by_name() {
        let current = stub_analysis(120, 45, &[(".text", 120)], &[("main", 80)]);
        let previous = stub_analysis(100, 40, &[(".text", 100)], &[("main", 60)]);
        let diff = diff_results(&current, &previous);
        assert_eq!(diff.rom_delta, 20);
        assert_eq!(diff.ram_delta, 5);
        assert_eq!(diff.section_diffs[0].name, ".text");
        assert_eq!(diff.symbol_diffs[0].delta, 20);
        assert_eq!(diff.symbol_diffs[0].change, DiffChangeKind::Increased);
    }

    #[test]
    fn emits_threshold_and_growth_warnings() {
        let mut current = stub_analysis(90, 50, &[(".data", 42), (".bss", 50)], &[("blob", 5000)]);
        current.memory.memory_regions = vec![
            MemoryRegion {
                name: "rom".to_string(),
                origin: 0,
                length: 100,
                attributes: "xr".to_string(),
            },
            MemoryRegion {
                name: "ram".to_string(),
                origin: 0x2000_0000,
                length: 55,
                attributes: "xrw".to_string(),
            },
        ];
        let diff = DiffResult {
            rom_delta: 10,
            ram_delta: 8,
            summary: DiffSummary::default(),
            section_diffs: vec![
                crate::model::DiffEntry {
                    name: ".data".to_string(),
                    current: 42,
                    previous: 20,
                    delta: 22,
                    change: DiffChangeKind::Increased,
                },
                crate::model::DiffEntry {
                    name: ".bss".to_string(),
                    current: 50,
                    previous: 40,
                    delta: 10,
                    change: DiffChangeKind::Increased,
                },
            ],
            symbol_diffs: vec![crate::model::DiffEntry {
                name: "blob".to_string(),
                current: 5000,
                previous: 0,
                delta: 5000,
                change: DiffChangeKind::Added,
            }],
            object_diffs: Vec::new(),
            archive_diffs: Vec::new(),
        };
        let warnings = evaluate_warnings(&current, Some(&diff));
        assert!(warnings.iter().any(|w| w.code == "ROM_THRESHOLD"));
        assert!(warnings.iter().any(|w| w.code == "RAM_THRESHOLD"));
        assert!(warnings.iter().any(|w| w.code == "DATA_GROWTH"));
        assert!(warnings.iter().any(|w| w.code == "SYMBOL_SPIKE"));
    }

    fn stub_analysis(rom: u64, ram: u64, sections: &[(&str, u64)], symbols: &[(&str, u64)]) -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "a.elf".to_string(),
                arch: "ARM".to_string(),
                elf_class: "ELF32".to_string(),
                endian: "little-endian".to_string(),
            },
            sections: Vec::new(),
            symbols: symbols
                .iter()
                .map(|(name, size)| SymbolInfo {
                    name: (*name).to_string(),
                    demangled_name: None,
                    section_name: None,
                    object_path: None,
                    size: *size,
                })
                .collect(),
            object_contributions: Vec::new(),
            archive_contributions: Vec::new(),
            memory: MemorySummary {
                rom_bytes: rom,
                ram_bytes: ram,
                section_totals: sections
                    .iter()
                    .map(|(name, size)| SectionTotal {
                        section_name: (*name).to_string(),
                        size: *size,
                        category: SectionCategory::Rom,
                    })
                    .collect(),
                memory_regions: Vec::new(),
            },
            warnings: Vec::new(),
        }
    }
}
