use crate::analyze::format_bytes;
use crate::model::{AnalysisResult, DiffResult, ThresholdConfig, WarningItem, WarningLevel, WarningSource};

pub trait Rule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult>;
}

pub struct RuleContext<'a> {
    pub current: &'a AnalysisResult,
    pub diff: Option<&'a DiffResult>,
    pub thresholds: &'a ThresholdConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleResult {
    pub code: String,
    pub severity: RuleSeverity,
    pub message: String,
    pub related: Option<String>,
}

pub fn evaluate_default_rules(context: &RuleContext<'_>) -> Vec<WarningItem> {
    let rules: [&dyn Rule; 9] = [
        &RomUsageHighRule,
        &RamUsageHighRule,
        &RegionUsageHighRule,
        &RegionFreeSpaceLowRule,
        &SectionRegionMismatchRule,
        &DataGrowthHighRule,
        &BssGrowthHighRule,
        &LargeSymbolRule,
        &LargeSymbolGrowthRule,
    ];
    rules
        .into_iter()
        .flat_map(|rule| rule.evaluate(context))
        .map(to_warning_item)
        .collect()
}

fn to_warning_item(result: RuleResult) -> WarningItem {
    WarningItem {
        level: match result.severity {
            RuleSeverity::Info => WarningLevel::Info,
            RuleSeverity::Warn => WarningLevel::Warn,
            RuleSeverity::Error => WarningLevel::Error,
        },
        code: result.code,
        message: result.message,
        source: WarningSource::Analyze,
        related: result.related,
    }
}

struct RomUsageHighRule;
struct RamUsageHighRule;
struct RegionUsageHighRule;
struct RegionFreeSpaceLowRule;
struct SectionRegionMismatchRule;
struct DataGrowthHighRule;
struct BssGrowthHighRule;
struct LargeSymbolRule;
struct LargeSymbolGrowthRule;

impl Rule for RomUsageHighRule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult> {
        let capacity = memory_capacity(&context.current.memory.memory_regions, &["rom", "flash"]);
        match capacity {
            Some(capacity) if capacity > 0 => {
                let ratio = context.current.memory.rom_bytes as f64 / capacity as f64 * 100.0;
                if ratio >= context.thresholds.rom_percent {
                    vec![RuleResult {
                        code: "ROM_THRESHOLD".to_string(),
                        severity: RuleSeverity::Warn,
                        message: format!("ROM usage exceeded {:.0}% ({:.1}%)", context.thresholds.rom_percent, ratio),
                        related: Some("rom".to_string()),
                    }]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }
}

impl Rule for RamUsageHighRule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult> {
        let capacity = memory_capacity(&context.current.memory.memory_regions, &["ram"]);
        match capacity {
            Some(capacity) if capacity > 0 => {
                let ratio = context.current.memory.ram_bytes as f64 / capacity as f64 * 100.0;
                if ratio >= context.thresholds.ram_percent {
                    vec![RuleResult {
                        code: "RAM_THRESHOLD".to_string(),
                        severity: RuleSeverity::Warn,
                        message: format!("RAM usage exceeded {:.0}% ({:.1}%)", context.thresholds.ram_percent, ratio),
                        related: Some("ram".to_string()),
                    }]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }
}

impl Rule for RegionUsageHighRule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult> {
        context
            .current
            .memory
            .region_summaries
            .iter()
            .filter_map(|region| {
                let threshold = context
                    .thresholds
                    .region_percent
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(&region.region_name))
                    .map(|(_, value)| *value)
                    .unwrap_or(context.thresholds.region_default_percent);
                let usage = region.usage_ratio * 100.0;
                (usage >= threshold).then(|| RuleResult {
                    code: "REGION_THRESHOLD".to_string(),
                    severity: RuleSeverity::Warn,
                    message: format!("Region {} usage exceeded {:.0}% ({:.1}%)", region.region_name, threshold, usage),
                    related: Some(region.region_name.clone()),
                })
            })
            .collect()
    }
}

impl Rule for RegionFreeSpaceLowRule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult> {
        context
            .current
            .memory
            .region_summaries
            .iter()
            .filter(|region| region.free <= context.thresholds.region_low_free_bytes)
            .map(|region| RuleResult {
                code: "REGION_LOW_FREE".to_string(),
                severity: RuleSeverity::Warn,
                message: format!("Region {} free space is low ({})", region.region_name, format_bytes(region.free)),
                related: Some(region.region_name.clone()),
            })
            .collect()
    }
}

impl Rule for SectionRegionMismatchRule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult> {
        let Some(lds) = &context.current.linker_script else {
            return Vec::new();
        };
        lds.placements
            .iter()
            .filter_map(|placement| {
                let section = context.current.sections.iter().find(|section| section.name == placement.section_name)?;
                let region = lds.regions.iter().find(|region| region.name == placement.region_name)?;
                let in_range = section.addr >= region.origin
                    && section.addr.saturating_add(section.size) <= region.origin.saturating_add(region.length);
                (!in_range).then(|| RuleResult {
                    code: "SECTION_REGION_MISMATCH".to_string(),
                    severity: RuleSeverity::Warn,
                    message: format!(
                        "Section {} is assigned to region {} but its address is outside the region range",
                        section.name, region.name
                    ),
                    related: Some(section.name.clone()),
                })
            })
            .collect()
    }
}

impl Rule for DataGrowthHighRule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult> {
        section_growth_result(context, ".data", "DATA_GROWTH")
    }
}

impl Rule for BssGrowthHighRule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult> {
        section_growth_result(context, ".bss", "BSS_GROWTH")
    }
}

impl Rule for LargeSymbolRule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult> {
        context
            .current
            .symbols
            .iter()
            .filter(|symbol| symbol.size >= context.thresholds.large_symbol_bytes)
            .take(5)
            .map(|symbol| RuleResult {
                code: "LARGE_SYMBOL".to_string(),
                severity: RuleSeverity::Warn,
                message: format!("Large symbol detected: {} ({})", symbol.name, format_bytes(symbol.size)),
                related: Some(symbol.name.clone()),
            })
            .collect()
    }
}

impl Rule for LargeSymbolGrowthRule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult> {
        let Some(diff) = context.diff else {
            return Vec::new();
        };
        diff.symbol_diffs
            .iter()
            .find(|entry| entry.delta >= context.thresholds.symbol_growth_bytes as i64)
            .map(|entry| {
                vec![RuleResult {
                    code: "SYMBOL_SPIKE".to_string(),
                    severity: RuleSeverity::Warn,
                    message: format!("Symbol growth spike: {} ({:+})", entry.name, entry.delta),
                    related: Some(entry.name.clone()),
                }]
            })
            .unwrap_or_default()
    }
}

fn section_growth_result(context: &RuleContext<'_>, section_name: &str, code: &str) -> Vec<RuleResult> {
    let Some(diff) = context.diff else {
        return Vec::new();
    };
    diff.section_diffs
        .iter()
        .find(|entry| entry.name == section_name && entry.previous > 0)
        .and_then(|entry| {
            let growth = entry.delta as f64 / entry.previous as f64 * 100.0;
            (growth >= context.thresholds.section_growth_rate).then(|| RuleResult {
                code: code.to_string(),
                severity: RuleSeverity::Warn,
                message: format!("{section_name} grew by {:.1}% ({:+})", growth, entry.delta),
                related: Some(section_name.to_string()),
            })
        })
        .into_iter()
        .collect()
}

fn memory_capacity(regions: &[crate::model::MemoryRegion], names: &[&str]) -> Option<u64> {
    let total = regions
        .iter()
        .filter(|region| names.iter().any(|name| region.name.eq_ignore_ascii_case(name)))
        .map(|region| region.length)
        .sum::<u64>();
    (total > 0).then_some(total)
}

#[cfg(test)]
mod tests {
    use super::{evaluate_default_rules, RuleContext};
    use crate::model::{
        AnalysisResult, BinaryInfo, DiffChangeKind, DiffEntry, DiffResult, DiffSummary, LinkerScriptInfo, MemoryRegion,
        MemorySummary, RegionUsageSummary, SectionCategory, SectionInfo, SectionPlacement, SectionTotal, SymbolInfo,
        ThresholdConfig,
    };

    #[test]
    fn rule_engine_emits_expected_rule_codes() {
        let mut current = stub_analysis();
        current.sections = vec![SectionInfo {
            name: ".data".to_string(),
            addr: 0x1000,
            size: 42,
            flags: vec![],
            category: SectionCategory::Ram,
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
        let diff = DiffResult {
            rom_delta: 10,
            ram_delta: 8,
            summary: DiffSummary::default(),
            section_diffs: vec![DiffEntry {
                name: ".data".to_string(),
                current: 42,
                previous: 20,
                delta: 22,
                change: DiffChangeKind::Increased,
            }],
            symbol_diffs: vec![DiffEntry {
                name: "blob".to_string(),
                current: 5000,
                previous: 0,
                delta: 5000,
                change: DiffChangeKind::Added,
            }],
            object_diffs: Vec::new(),
            archive_diffs: Vec::new(),
        };
        let context = RuleContext {
            current: &current,
            diff: Some(&diff),
            thresholds: &ThresholdConfig::default(),
        };
        let warnings = evaluate_default_rules(&context);
        for code in [
            "ROM_THRESHOLD",
            "RAM_THRESHOLD",
            "REGION_THRESHOLD",
            "REGION_LOW_FREE",
            "SECTION_REGION_MISMATCH",
            "DATA_GROWTH",
            "LARGE_SYMBOL",
            "SYMBOL_SPIKE",
        ] {
            assert!(warnings.iter().any(|warning| warning.code == code), "missing {code}");
        }
    }

    #[test]
    fn rule_engine_respects_custom_thresholds() {
        let current = stub_analysis();
        let diff = DiffResult {
            rom_delta: 0,
            ram_delta: 0,
            summary: DiffSummary::default(),
            section_diffs: vec![DiffEntry {
                name: ".data".to_string(),
                current: 42,
                previous: 41,
                delta: 1,
                change: DiffChangeKind::Increased,
            }],
            symbol_diffs: vec![DiffEntry {
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
        let context = RuleContext {
            current: &current,
            diff: Some(&diff),
            thresholds: &thresholds,
        };
        let warnings = evaluate_default_rules(&context);
        assert!(!warnings.iter().any(|warning| warning.code == "SYMBOL_SPIKE"));
        assert!(!warnings.iter().any(|warning| warning.code == "DATA_GROWTH"));
    }

    fn stub_analysis() -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "a.elf".to_string(),
                arch: "ARM".to_string(),
                elf_class: "ELF32".to_string(),
                endian: "little-endian".to_string(),
            },
            sections: Vec::new(),
            symbols: vec![SymbolInfo {
                name: "blob".to_string(),
                demangled_name: None,
                section_name: None,
                object_path: None,
                size: 5000,
            }],
            object_contributions: Vec::new(),
            archive_contributions: Vec::new(),
            linker_script: None,
            memory: MemorySummary {
                rom_bytes: 90,
                ram_bytes: 50,
                section_totals: vec![SectionTotal {
                    section_name: ".data".to_string(),
                    size: 42,
                    category: SectionCategory::Ram,
                }],
                memory_regions: vec![
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
                ],
                region_summaries: vec![RegionUsageSummary {
                    region_name: "RAM".to_string(),
                    origin: 0x2000_0000,
                    length: 55,
                    used: 52,
                    free: 3,
                    usage_ratio: 52.0 / 55.0,
                    sections: Vec::new(),
                }],
            },
            warnings: Vec::new(),
        }
    }
}
