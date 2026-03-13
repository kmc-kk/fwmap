use std::collections::BTreeMap;
use std::path::Path;

use crate::cpp::build_cpp_view;
use crate::debug::{resolve_debug_artifact, DebugArtifactResolver};
use crate::demangle::apply_demangling;
use crate::ingest::{dwarf, elf, lds, map};
use crate::model::{
    AnalysisResult, ArchiveContribution, CustomRule, DebuginfodMode, DemangleMode, DiffResult,
    DwarfMode, FunctionAttribution, LineAttribution, LineRangeAttribution, LinkerFamily, MapFormat,
    MapFormatSelection, MemoryRegion, MemorySummary, ObjectContribution, ObjectSourceKind, RegionSectionUsage,
    RegionUsageSummary, SectionCategory, SectionInfo, SectionPlacement, SectionTotal, SourceFile, SourceLinesMode,
    SourceSpan, SymbolInfo, ThresholdConfig, ToolchainInfo, ToolchainKind, ToolchainSelection, WarningItem,
};
use crate::rules::{evaluate_default_rules, RuleContext};
use crate::validation::quality::evaluate_quality_checks;

#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    pub thresholds: ThresholdConfig,
    pub demangle: DemangleMode,
    pub custom_rules: Vec<CustomRule>,
    pub toolchain: ToolchainSelection,
    pub map_format: MapFormatSelection,
    pub dwarf_mode: DwarfMode,
    pub debug_file_dirs: Vec<std::path::PathBuf>,
    pub debug_trace: bool,
    pub debuginfod: DebuginfodMode,
    pub debuginfod_urls: Vec<String>,
    pub debuginfod_cache_dir: Option<std::path::PathBuf>,
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
            map_format: MapFormatSelection::Auto,
            dwarf_mode: DwarfMode::Auto,
            debug_file_dirs: Vec::new(),
            debug_trace: false,
            debuginfod: DebuginfodMode::Off,
            debuginfod_urls: Vec::new(),
            debuginfod_cache_dir: None,
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
        Some(path) => Some(map::parse_map(path, options.toolchain, options.map_format)?),
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
    let resolved_debug = resolve_debug_artifact(
        elf_path,
        &DebugArtifactResolver {
            debug_file_dirs: options.debug_file_dirs.clone(),
            debuginfod: options.debuginfod,
            debuginfod_urls: options.debuginfod_urls.clone(),
            debuginfod_cache_dir: options.debuginfod_cache_dir.clone(),
            trace: options.debug_trace,
        },
    )?;
    let dwarf_data = dwarf::parse_dwarf(elf_path, &resolved_debug, &elf.sections, options)?;
    // Rebuild source aggregates after demangling so reports and diffs share one normalized view.
    let function_attributions = match options.source_lines {
        SourceLinesMode::Functions | SourceLinesMode::Lines | SourceLinesMode::All => {
            aggregate_function_attributions(&dwarf_data.line_attributions, &symbols)
        }
        _ => Vec::new(),
    };
    let source_files = aggregate_source_files(&dwarf_data.line_attributions, &function_attributions);
    let line_hotspots = match options.source_lines {
        SourceLinesMode::Lines | SourceLinesMode::All => aggregate_line_hotspots(&dwarf_data.line_attributions),
        _ => Vec::new(),
    };

    let mut warnings = elf.warnings;
    if let Some(map_data) = map_data.as_ref() {
        warnings.extend(map_data.warnings.clone());
    }
    if let Some(lds_data) = lds_data.as_ref() {
        warnings.extend(lds_data.warnings.clone());
    }
    warnings.extend(dwarf_data.warnings.clone());

    let sorted_symbols = sorted_symbols(symbols);
    let cpp_view = build_cpp_view(&sorted_symbols);

    let mut result = AnalysisResult {
        binary: elf.binary,
        toolchain: ToolchainInfo {
            requested: options.toolchain,
            detected: map_data.as_ref().and_then(|item| item.detected_toolchain),
            resolved: map_data
                .as_ref()
                .map(|item| item.resolved_toolchain)
                .unwrap_or_else(|| resolve_toolchain_without_map(options.toolchain)),
            linker_family: map_data
                .as_ref()
                .map(|item| item.linker_family)
                .unwrap_or(LinkerFamily::Unknown),
            map_format: map_data.as_ref().map(|item| item.map_format).unwrap_or(MapFormat::Unknown),
            parser_warnings_count: map_data.as_ref().map(|item| item.parser_warnings_count()).unwrap_or(0),
        },
        debug_info: dwarf_data.debug_info,
        debug_artifact: dwarf_data.debug_artifact,
        sections: elf.sections,
        symbols: sorted_symbols,
        object_contributions: aggregate_objects(map_data.as_ref().map(|item| item.object_contributions.as_slice()).unwrap_or(&[])),
        archive_contributions: aggregate_archives(map_data.as_ref().map(|item| item.archive_contributions.as_slice()).unwrap_or(&[])),
        archive_pulls: map_data.as_ref().map(|item| item.archive_pulls.clone()).unwrap_or_default(),
        whole_archive_candidates: map_data
            .as_ref()
            .map(|item| item.whole_archive_candidates.clone())
            .unwrap_or_default(),
        relocation_references: elf.relocation_references,
        cross_references: map_data.map(|item| item.cross_references).unwrap_or_default(),
        cpp_view,
        linker_script: lds_data.map(|item| item.linker_script),
        memory,
        compilation_units: dwarf_data.compilation_units,
        source_files,
        line_attributions: dwarf_data.line_attributions,
        line_hotspots,
        function_attributions,
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

fn aggregate_source_files(lines: &[LineAttribution], functions: &[FunctionAttribution]) -> Vec<SourceFile> {
    let mut totals = BTreeMap::<String, (u64, std::collections::BTreeSet<(u64, u64)>)>::new();
    for line in lines {
        let entry = totals
            .entry(line.location.path.clone())
            .or_insert_with(|| (0, std::collections::BTreeSet::new()));
        entry.0 += line.size;
        entry.1.insert((line.span.line_start, line.span.line_end));
    }
    let mut functions_per_path = BTreeMap::<String, usize>::new();
    for function in functions {
        if let Some(path) = function.path.as_ref() {
            *functions_per_path.entry(path.clone()).or_default() += 1;
        }
    }
    let mut files = totals
        .into_iter()
        .map(|(path, (size, ranges))| SourceFile {
            directory: std::path::Path::new(&path)
                .parent()
                .map(|item| item.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default(),
            display_path: path.clone(),
            path: path.clone(),
            size,
            functions: functions_per_path.get(&path).copied().unwrap_or(0),
            line_ranges: ranges.len(),
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    files
}

fn aggregate_function_attributions(lines: &[LineAttribution], symbols: &[SymbolInfo]) -> Vec<FunctionAttribution> {
    if lines.is_empty() || symbols.is_empty() {
        return Vec::new();
    }
    let mut sorted_lines = lines.iter().collect::<Vec<_>>();
    sorted_lines.sort_by(|a, b| {
        a.range
            .start
            .cmp(&b.range.start)
            .then_with(|| a.range.end.cmp(&b.range.end))
    });
    let mut sorted_symbols = symbols
        .iter()
        .filter(|symbol| symbol.size > 0)
        .collect::<Vec<_>>();
    sorted_symbols.sort_by(|a, b| a.addr.cmp(&b.addr).then_with(|| a.name.cmp(&b.name)));

    let mut line_start = 0usize;
    let mut functions = Vec::new();
    for symbol in sorted_symbols {
        let symbol_start = symbol.addr;
        let symbol_end = symbol.addr.saturating_add(symbol.size);

        while line_start < sorted_lines.len() && sorted_lines[line_start].range.end <= symbol_start {
            line_start += 1;
        }

        let mut size = 0u64;
        let mut ranges = Vec::<SourceSpan>::new();
        let mut path = None;
        let mut index = line_start;
        while index < sorted_lines.len() {
            let line = sorted_lines[index];
            if line.range.start >= symbol_end {
                break;
            }
            // Attribute bytes by address overlap so optimized code still contributes to the owning symbol.
            let overlap_start = symbol_start.max(line.range.start);
            let overlap_end = symbol_end.min(line.range.end);
            if overlap_start < overlap_end {
                size += overlap_end - overlap_start;
                path.get_or_insert_with(|| line.location.path.clone());
                ranges.push(line.span.clone());
            }
            index += 1;
        }
        if size == 0 {
            continue;
        }
        functions.push(FunctionAttribution {
            raw_name: symbol.name.clone(),
            demangled_name: symbol.demangled_name.clone(),
            path,
            size,
            ranges: compress_source_spans(ranges),
        });
    }

    functions.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.raw_name.cmp(&b.raw_name))
    });
    functions
}

fn aggregate_line_hotspots(lines: &[LineAttribution]) -> Vec<LineRangeAttribution> {
    let mut grouped = BTreeMap::<(String, Option<String>), Vec<(u64, u64, u64)>>::new();
    for line in lines {
        grouped
            .entry((line.location.path.clone(), line.range.section_name.clone()))
            .or_default()
            .push((line.span.line_start, line.span.line_end, line.size));
    }
    let mut hotspots = Vec::new();
    for ((path, section_name), mut entries) in grouped {
        entries.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let mut merged: Vec<(u64, u64, u64)> = Vec::new();
        for (line_start, line_end, size) in entries {
            if let Some(last) = merged.last_mut() {
                if line_start <= last.1.saturating_add(1) {
                    last.1 = last.1.max(line_end);
                    last.2 += size;
                    continue;
                }
            }
            merged.push((line_start, line_end, size));
        }
        for (line_start, line_end, size) in merged {
            hotspots.push(LineRangeAttribution {
                path: path.clone(),
                line_start,
                line_end,
                section_name: section_name.clone(),
                size,
            });
        }
    }
    hotspots.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.line_start.cmp(&b.line_start))
    });
    hotspots
}

fn compress_source_spans(mut spans: Vec<SourceSpan>) -> Vec<SourceSpan> {
    if spans.is_empty() {
        return spans;
    }
    spans.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.line_start.cmp(&b.line_start))
            .then_with(|| a.line_end.cmp(&b.line_end))
    });
    let mut compressed: Vec<SourceSpan> = Vec::new();
    for span in spans {
        // Merge adjacent lines to keep HTML/CI output readable when DWARF emits one row per instruction group.
        if let Some(last) = compressed.last_mut() {
            if last.path == span.path && span.line_start <= last.line_end.saturating_add(1) {
                last.line_end = last.line_end.max(span.line_end);
                continue;
            }
        }
        compressed.push(span);
    }
    compressed
}

fn aggregate_objects(items: &[ObjectContribution]) -> Vec<ObjectContribution> {
    let mut totals = BTreeMap::<(String, ObjectSourceKind, Option<String>), u64>::new();
    for item in items {
        *totals
            .entry((item.object_path.clone(), item.source_kind, item.section_name.clone()))
            .or_default() += item.size;
    }
    let mut result = totals
        .into_iter()
        .map(|((object_path, source_kind, section_name), size)| ObjectContribution {
            object_path,
            source_kind,
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
        AnalysisResult, BinaryInfo, DebugArtifactInfo, DebugInfoSummary, DiffChangeKind, DiffResult, DiffSummary,
        DwarfMode, FunctionAttribution, LineAttribution, LinkerFamily, LinkerScriptInfo, MapFormat, MemoryRegion,
        MemorySummary, SectionCategory, SectionInfo, SectionPlacement, SectionTotal, SourceLocation, SourceSpan,
        SymbolInfo, ThresholdConfig, ToolchainInfo, ToolchainKind, ToolchainSelection, UnknownSourceBucket,
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
                addr: 0,
                size: 1,
            },
            SymbolInfo {
                name: "big".to_string(),
                demangled_name: None,
                section_name: None,
                object_path: None,
                addr: 0,
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
            unknown_source_delta: 0,
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
            source_file_diffs: Vec::new(),
            function_diffs: Vec::new(),
            line_diffs: Vec::new(),
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
            unknown_source_delta: 0,
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
            source_file_diffs: Vec::new(),
            function_diffs: Vec::new(),
            line_diffs: Vec::new(),
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

    #[test]
    fn aggregates_functions_and_hotspots_from_line_ranges() {
        let symbols = vec![
            SymbolInfo {
                name: "_ZN3app4tickEv".to_string(),
                demangled_name: Some("app::tick()".to_string()),
                section_name: Some(".text".to_string()),
                object_path: None,
                addr: 0x1000,
                size: 12,
            },
            SymbolInfo {
                name: "helper".to_string(),
                demangled_name: None,
                section_name: Some(".text".to_string()),
                object_path: None,
                addr: 0x2000,
                size: 4,
            },
        ];
        let lines = vec![
            LineAttribution {
                location: SourceLocation {
                    path: "src/main.cpp".to_string(),
                    line: 10,
                    column: None,
                },
                span: SourceSpan {
                    path: "src/main.cpp".to_string(),
                    line_start: 10,
                    line_end: 10,
                    column: None,
                },
                range: crate::model::AddressRange {
                    start: 0x1000,
                    end: 0x1004,
                    section_name: Some(".text".to_string()),
                },
                size: 4,
            },
            LineAttribution {
                location: SourceLocation {
                    path: "src/main.cpp".to_string(),
                    line: 11,
                    column: None,
                },
                span: SourceSpan {
                    path: "src/main.cpp".to_string(),
                    line_start: 11,
                    line_end: 11,
                    column: None,
                },
                range: crate::model::AddressRange {
                    start: 0x1004,
                    end: 0x100c,
                    section_name: Some(".text".to_string()),
                },
                size: 8,
            },
        ];
        let functions = super::aggregate_function_attributions(&lines, &symbols);
        assert_eq!(functions[0].raw_name, "_ZN3app4tickEv");
        assert_eq!(functions[0].size, 12);
        assert_eq!(functions[0].ranges.len(), 1);

        let hotspots = super::aggregate_line_hotspots(&lines);
        assert_eq!(hotspots[0].path, "src/main.cpp");
        assert_eq!(hotspots[0].line_start, 10);
        assert_eq!(hotspots[0].line_end, 11);

        let files = super::aggregate_source_files(&lines, &functions);
        assert_eq!(files[0].functions, 1);
        assert_eq!(files[0].line_ranges, 2);
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
                linker_family: LinkerFamily::Gnu,
                map_format: MapFormat::Unknown,
                parser_warnings_count: 0,
            },
            debug_info: DebugInfoSummary::default(),
            debug_artifact: DebugArtifactInfo::default(),
            sections: Vec::new(),
            symbols: symbols
                .iter()
                .map(|(name, size)| SymbolInfo {
                    name: (*name).to_string(),
                    demangled_name: None,
                    section_name: None,
                    object_path: None,
                    addr: 0,
                    size: *size,
                })
                .collect(),
            object_contributions: Vec::new(),
            archive_contributions: Vec::new(),
            archive_pulls: Vec::new(),
            whole_archive_candidates: Vec::new(),
            relocation_references: Vec::new(),
            cross_references: Vec::new(),
            cpp_view: crate::model::CppView::default(),
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
            line_hotspots: Vec::new(),
            function_attributions: Vec::<FunctionAttribution>::new(),
            unknown_source: UnknownSourceBucket::default(),
            warnings: Vec::new(),
        }
    }
}
