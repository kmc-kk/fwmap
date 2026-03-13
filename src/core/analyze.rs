use std::collections::BTreeMap;
use std::path::Path;

use crate::demangle::apply_demangling;
use crate::ingest::{dwarf, elf, lds, map};
use crate::model::{
    AnalysisResult, ArchiveContribution, CustomRule, DemangleMode, DiffResult, DwarfMode, MemoryRegion, MemorySummary,
    ObjectContribution, RegionSectionUsage, RegionUsageSummary, SectionCategory, SectionInfo, SectionPlacement,
    SectionTotal, SourceLinesMode, SymbolInfo, ThresholdConfig, ToolchainInfo, ToolchainKind, ToolchainSelection,
    WarningItem,
};
use crate::rules::{evaluate_default_rules, RuleContext};
use crate::validation::quality::evaluate_quality_checks;

#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    pub thresholds: ThresholdConfig,
    pub demangle: DemangleMode,
    pub custom_rules: Vec<CustomRule>,
    pub toolchain: ToolchainSelection,
    pub dwarf_mode: DwarfMode,
    pub source_lines: SourceLinesMode,
    pub source_root: Option<std::path::PathBuf>,
    pub path_remaps: Vec<(String, String)>,
    pub fail_on_missing_dwarf: bool,
}

impl Default for AnalyzeOptions {
    fn default() -> Self {
        Self {
            thresholds: ThresholdConfig::default(),
            demangle: DemangleMode::Auto,
            custom_rules: Vec::new(),
            toolchain: ToolchainSelection::Auto,
            dwarf_mode: DwarfMode::Auto,
            source_lines: SourceLinesMode::Off,
            source_root: None,
            path_remaps: Vec::new(),
            fail_on_missing_dwarf: false,
        }
    }
}

pub fn analyze_paths(
    elf_path: &Path,
    map_path: Option<&Path>,
    lds_path: Option<&Path>,
    options: &AnalyzeOptions,
) -> Result<AnalysisResult, String> {
    let elf = elf::parse_elf(elf_path)?;
    let map_data = match map_path {
        Some(path) => Some(map::parse_map(path, options.toolchain)?),
        None => None,
    };
    let lds_data = match lds_path {
        Some(path) => Some(lds::parse_lds(path)?),
        None => None,
    };
    let region_input = lds_data
        .as_ref()
        .map(|item| item.linker_script.regions.as_slice())
        .or_else(|| map_data.as_ref().map(|item| item.memory_regions.as_slice()))
        .unwrap_or(&[]);
    let placements = lds_data
        .as_ref()
        .map(|item| item.linker_script.placements.as_slice())
        .unwrap_or(&[]);
    let memory = build_memory_summary(&elf.sections, region_input, placements);
    let mut symbols = elf.symbols;
    apply_demangling(&mut symbols, options.demangle);
    let dwarf_data = dwarf::parse_dwarf(elf_path, &elf.sections, options)?;

    let mut warnings = elf.warnings;
    if let Some(map_data) = map_data.as_ref() {
        warnings.extend(map_data.warnings.clone());
    }
    if let Some(lds_data) = lds_data.as_ref() {
        warnings.extend(lds_data.warnings.clone());
    }
    warnings.extend(dwarf_data.warnings.clone());

    let mut result = AnalysisResult {
        binary: elf.binary,
        toolchain: ToolchainInfo {
            requested: options.toolchain,
            detected: map_data.as_ref().and_then(|item| item.detected_toolchain),
            resolved: map_data
                .as_ref()
                .map(|item| item.resolved_toolchain)
                .unwrap_or_else(|| resolve_toolchain_without_map(options.toolchain)),
        },
        debug_info: dwarf_data.debug_info,
        sections: elf.sections,
        symbols: sorted_symbols(symbols),
        object_contributions: aggregate_objects(map_data.as_ref().map(|item| item.object_contributions.as_slice()).unwrap_or(&[])),
        archive_contributions: aggregate_archives(map_data.as_ref().map(|item| item.archive_contributions.as_slice()).unwrap_or(&[])),
        linker_script: lds_data.map(|item| item.linker_script),
        memory,
        compilation_units: dwarf_data.compilation_units,
        source_files: dwarf_data.source_files,
        line_attributions: dwarf_data.line_attributions,
        function_attributions: dwarf_data.function_attributions,
        unknown_source: dwarf_data.unknown_source,
        warnings,
    };
    result.warnings.extend(evaluate_quality_checks(&result));
    result.warnings.extend(evaluate_warnings(&result, None, &options.thresholds, &options.custom_rules));
    Ok(result)
}

fn resolve_toolchain_without_map(selection: ToolchainSelection) -> ToolchainKind {
    match selection {
        ToolchainSelection::Lld => ToolchainKind::Lld,
        _ => ToolchainKind::Gnu,
    }
}

pub fn build_memory_summary(sections: &[SectionInfo], regions: &[MemoryRegion], placements: &[SectionPlacement]) -> MemorySummary {
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
    let mut region_summaries = build_region_summaries(sections, regions, placements);
    region_summaries.sort_by(|a, b| b.used.cmp(&a.used).then_with(|| a.region_name.cmp(&b.region_name)));

    MemorySummary {
        rom_bytes,
        ram_bytes,
        section_totals: totals,
        memory_regions: regions.to_vec(),
        region_summaries,
    }
}

pub fn sorted_symbols(mut symbols: Vec<SymbolInfo>) -> Vec<SymbolInfo> {
    symbols.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    symbols
}

pub fn evaluate_warnings(
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    thresholds: &ThresholdConfig,
    custom_rules: &[CustomRule],
) -> Vec<WarningItem> {
    evaluate_default_rules(&RuleContext {
        current,
        diff,
        thresholds,
        custom_rules,
    })
}

pub fn format_bytes(bytes: u64) -> String {
    format!("{bytes} bytes ({:.2} KiB)", bytes as f64 / 1024.0)
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

fn build_region_summaries(
    sections: &[SectionInfo],
    regions: &[MemoryRegion],
    placements: &[SectionPlacement],
) -> Vec<RegionUsageSummary> {
    let mut summaries = Vec::new();
    for region in regions {
        let mut matched_sections = sections
            .iter()
            .filter(|section| section_in_region(section, region, placements))
            .map(|section| RegionSectionUsage {
                section_name: section.name.clone(),
                addr: section.addr,
                size: section.size,
            })
            .collect::<Vec<_>>();
        matched_sections.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.section_name.cmp(&b.section_name)));
        let used = matched_sections.iter().map(|section| section.size).sum::<u64>();
        let free = region.length.saturating_sub(used);
        let usage_ratio = if region.length > 0 { used as f64 / region.length as f64 } else { 0.0 };
        summaries.push(RegionUsageSummary {
            region_name: region.name.clone(),
            origin: region.origin,
            length: region.length,
            used,
            free,
            usage_ratio,
            sections: matched_sections,
        });
    }
    summaries
}

fn section_in_region(section: &SectionInfo, region: &MemoryRegion, placements: &[SectionPlacement]) -> bool {
    if let Some(placement) = placements.iter().find(|placement| placement.section_name == section.name) {
        if placement.region_name.eq_ignore_ascii_case(&region.name) {
            return true;
        }
    }
    section.addr >= region.origin && section.addr < region.origin.saturating_add(region.length)
}

#[cfg(test)]
mod tests {
    use super::{build_memory_summary, evaluate_warnings, sorted_symbols, AnalyzeOptions};
    use crate::diff::diff_results;
    use crate::model::{
        AnalysisResult, BinaryInfo, DebugInfoSummary, DiffChangeKind, DiffResult, DiffSummary, DwarfMode,
        FunctionAttribution, LinkerScriptInfo, MemoryRegion, MemorySummary, SectionCategory, SectionInfo,
        SectionPlacement, SectionTotal, SymbolInfo, ThresholdConfig, ToolchainInfo, ToolchainKind,
        ToolchainSelection, UnknownSourceBucket,
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
        let summary = build_memory_summary(&sections, &[], &[]);
        assert_eq!(summary.rom_bytes, 100);
        assert_eq!(summary.ram_bytes, 32);
    }

    #[test]
    fn builds_region_summary_from_placements() {
        let sections = vec![
            SectionInfo {
                name: ".text".to_string(),
                addr: 0x0800_0000,
                size: 100,
                flags: vec![],
                category: SectionCategory::Rom,
            },
            SectionInfo {
                name: ".data".to_string(),
                addr: 0x2000_0000,
                size: 20,
                flags: vec![],
                category: SectionCategory::Ram,
            },
        ];
        let regions = vec![
            MemoryRegion { name: "FLASH".to_string(), origin: 0x0800_0000, length: 256, attributes: "rx".to_string() },
            MemoryRegion { name: "RAM".to_string(), origin: 0x2000_0000, length: 128, attributes: "rwx".to_string() },
        ];
        let placements = vec![
            SectionPlacement { section_name: ".text".to_string(), region_name: "FLASH".to_string(), load_region_name: None, align: None, keep: false, has_at: false },
            SectionPlacement { section_name: ".data".to_string(), region_name: "RAM".to_string(), load_region_name: Some("FLASH".to_string()), align: None, keep: false, has_at: true },
        ];
        let summary = build_memory_summary(&sections, &regions, &placements);
        assert_eq!(summary.region_summaries.len(), 2);
        assert_eq!(summary.region_summaries[0].used, 100);
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
        current.memory.region_summaries = vec![crate::model::RegionUsageSummary {
            region_name: "RAM".to_string(),
            origin: 0x2000_0000,
            length: 55,
            used: 52,
            free: 3,
            usage_ratio: 52.0 / 55.0,
            sections: Vec::new(),
        }];
        current.linker_script = Some(LinkerScriptInfo {
            path: "test.ld".to_string(),
            regions: vec![MemoryRegion {
                name: "RAM".to_string(),
                origin: 0x2000_0000,
                length: 55,
                attributes: "xrw".to_string(),
            }],
            placements: vec![SectionPlacement {
                section_name: ".data".to_string(),
                region_name: "RAM".to_string(),
                load_region_name: None,
                align: None,
                keep: false,
                has_at: false,
            }],
        });
        current.sections = vec![SectionInfo {
            name: ".data".to_string(),
            addr: 0x1000,
            size: 42,
            flags: vec![],
            category: SectionCategory::Ram,
        }];
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
        let warnings = evaluate_warnings(&current, Some(&diff), &ThresholdConfig::default(), &[]);
        assert!(warnings.iter().any(|w| w.code == "ROM_THRESHOLD"));
        assert!(warnings.iter().any(|w| w.code == "RAM_THRESHOLD"));
        assert!(warnings.iter().any(|w| w.code == "REGION_THRESHOLD"));
        assert!(warnings.iter().any(|w| w.code == "REGION_LOW_FREE"));
        assert!(warnings.iter().any(|w| w.code == "DATA_GROWTH"));
        assert!(warnings.iter().any(|w| w.code == "SYMBOL_SPIKE"));
        assert!(warnings.iter().any(|w| w.code == "SECTION_REGION_MISMATCH"));
    }

    #[test]
    fn respects_custom_thresholds() {
        let current = stub_analysis(90, 50, &[(".data", 42)], &[("blob", 2048)]);
        let diff = DiffResult {
            rom_delta: 0,
            ram_delta: 0,
            summary: DiffSummary::default(),
            section_diffs: vec![crate::model::DiffEntry {
                name: ".data".to_string(),
                current: 42,
                previous: 41,
                delta: 1,
                change: DiffChangeKind::Increased,
            }],
            symbol_diffs: vec![crate::model::DiffEntry {
                name: "blob".to_string(),
                current: 2048,
                previous: 0,
                delta: 2048,
                change: DiffChangeKind::Added,
            }],
            object_diffs: Vec::new(),
            archive_diffs: Vec::new(),
        };
        let thresholds = ThresholdConfig {
            rom_percent: 95.0,
            ram_percent: 95.0,
            region_default_percent: 95.0,
            symbol_growth_bytes: 4096,
            section_growth_rate: 10.0,
            ..ThresholdConfig::default()
        };
        let warnings = evaluate_warnings(&current, Some(&diff), &thresholds, &[]);
        assert!(!warnings.iter().any(|w| w.code == "SYMBOL_SPIKE"));
        assert!(!warnings.iter().any(|w| w.code == "DATA_GROWTH"));
    }

    #[test]
    fn analyze_options_default_matches_previous_behavior() {
        let options = AnalyzeOptions::default();
        assert_eq!(options.thresholds.rom_percent, ThresholdConfig::default().rom_percent);
        assert!(options.custom_rules.is_empty());
        assert_eq!(options.toolchain, ToolchainSelection::Auto);
        assert_eq!(options.dwarf_mode, DwarfMode::Auto);
    }

    fn stub_analysis(rom: u64, ram: u64, sections: &[(&str, u64)], symbols: &[(&str, u64)]) -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "a.elf".to_string(),
                arch: "ARM".to_string(),
                elf_class: "ELF32".to_string(),
                endian: "little-endian".to_string(),
            },
            toolchain: ToolchainInfo {
                requested: ToolchainSelection::Auto,
                detected: None,
                resolved: ToolchainKind::Gnu,
            },
            debug_info: DebugInfoSummary::default(),
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
            linker_script: None,
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
                region_summaries: Vec::new(),
            },
            compilation_units: Vec::new(),
            source_files: Vec::new(),
            line_attributions: Vec::new(),
            function_attributions: Vec::<FunctionAttribution>::new(),
            unknown_source: UnknownSourceBucket::default(),
            warnings: Vec::new(),
        }
    }
}
