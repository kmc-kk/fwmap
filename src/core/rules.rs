use crate::analyze::format_bytes;
use crate::model::{
    AnalysisResult, CustomRule, DiffResult, RuleKind, RuleSeverityConfig, ThresholdConfig, WarningItem, WarningLevel,
    WarningSource,
};

pub trait Rule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult>;
}

pub struct RuleContext<'a> {
    pub current: &'a AnalysisResult,
    pub diff: Option<&'a DiffResult>,
    pub thresholds: &'a ThresholdConfig,
    pub custom_rules: &'a [CustomRule],
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
    let rules: [&dyn Rule; 10] = [
        &RomUsageHighRule,
        &RamUsageHighRule,
        &UnknownSourceRatioRule,
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
        .chain(evaluate_custom_rules(context))
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

fn evaluate_custom_rules(context: &RuleContext<'_>) -> Vec<RuleResult> {
    context
        .custom_rules
        .iter()
        .filter(|rule| rule.enabled)
        .flat_map(|rule| evaluate_custom_rule(context, rule))
        .collect()
}

fn evaluate_custom_rule(context: &RuleContext<'_>, rule: &CustomRule) -> Vec<RuleResult> {
    match rule.kind {
        RuleKind::RegionUsage => evaluate_custom_region_usage(context, rule),
        RuleKind::SectionDelta => evaluate_custom_section_delta(context, rule),
        RuleKind::SymbolDelta => evaluate_custom_symbol_delta(context, rule),
        RuleKind::SymbolMatch => evaluate_custom_symbol_match(context, rule),
        RuleKind::ObjectMatch => evaluate_custom_object_match(context, rule),
        RuleKind::SourcePathGrowth => evaluate_custom_source_path_growth(context, rule),
        RuleKind::FunctionGrowth => evaluate_custom_function_growth(context, rule),
        RuleKind::UnknownSourceRatio => evaluate_custom_unknown_source_ratio(context, rule),
    }
}

fn evaluate_custom_region_usage(context: &RuleContext<'_>, rule: &CustomRule) -> Vec<RuleResult> {
    let Some(region_name) = rule.region.as_deref() else {
        return Vec::new();
    };
    let Some(threshold) = rule.warn_if_greater_than else {
        return Vec::new();
    };
    let threshold = normalize_ratio_or_percent(threshold);
    context
        .current
        .memory
        .region_summaries
        .iter()
        .filter(|region| region.region_name.eq_ignore_ascii_case(region_name))
        .filter(|region| apply_name_filters(&region.region_name, &rule.allowlist, &rule.denylist))
        .filter_map(|region| {
            let usage = region.usage_ratio * 100.0;
            (usage >= threshold).then(|| custom_rule_result(rule, Some(region.region_name.clone())))
        })
        .collect()
}

fn evaluate_custom_section_delta(context: &RuleContext<'_>, rule: &CustomRule) -> Vec<RuleResult> {
    let Some(diff) = context.diff else {
        return Vec::new();
    };
    let Some(section_name) = rule.section.as_deref() else {
        return Vec::new();
    };
    let Some(threshold) = rule.warn_if_delta_bytes_gt else {
        return Vec::new();
    };
    diff.section_diffs
        .iter()
        .filter(|entry| entry.name == section_name)
        .filter(|entry| entry.delta > threshold)
        .filter(|entry| apply_name_filters(&entry.name, &rule.allowlist, &rule.denylist))
        .map(|entry| custom_rule_result(rule, Some(entry.name.clone())))
        .collect()
}

fn evaluate_custom_symbol_delta(context: &RuleContext<'_>, rule: &CustomRule) -> Vec<RuleResult> {
    let Some(diff) = context.diff else {
        return Vec::new();
    };
    let Some(symbol_name) = rule.symbol.as_deref() else {
        return Vec::new();
    };
    let Some(threshold) = rule.warn_if_delta_bytes_gt else {
        return Vec::new();
    };
    diff.symbol_diffs
        .iter()
        .filter(|entry| entry.name == symbol_name)
        .filter(|entry| entry.delta > threshold)
        .filter(|entry| apply_name_filters(&entry.name, &rule.allowlist, &rule.denylist))
        .map(|entry| custom_rule_result(rule, Some(entry.name.clone())))
        .collect()
}

fn evaluate_custom_symbol_match(context: &RuleContext<'_>, rule: &CustomRule) -> Vec<RuleResult> {
    let Some(symbol_name) = rule.symbol.as_deref() else {
        return Vec::new();
    };
    context
        .current
        .symbols
        .iter()
        .filter(|symbol| symbol.name == symbol_name)
        .filter(|symbol| apply_name_filters(&symbol.name, &rule.allowlist, &rule.denylist))
        .map(|symbol| custom_rule_result(rule, Some(symbol.name.clone())))
        .collect()
}

fn evaluate_custom_object_match(context: &RuleContext<'_>, rule: &CustomRule) -> Vec<RuleResult> {
    let Some(object_name) = rule.object.as_deref() else {
        return Vec::new();
    };
    context
        .current
        .object_contributions
        .iter()
        .filter(|item| item.object_path == object_name)
        .filter(|item| apply_name_filters(&item.object_path, &rule.allowlist, &rule.denylist))
        .map(|item| custom_rule_result(rule, Some(item.object_path.clone())))
        .collect()
}

fn evaluate_custom_source_path_growth(context: &RuleContext<'_>, rule: &CustomRule) -> Vec<RuleResult> {
    let Some(diff) = context.diff else {
        return Vec::new();
    };
    let Some(pattern) = rule.pattern.as_deref() else {
        return Vec::new();
    };
    let threshold = rule.threshold_bytes.or(rule.warn_if_delta_bytes_gt).unwrap_or_default();
    diff.source_file_diffs
        .iter()
        // Source rules operate on the normalized diff keys so one pattern works across CLI, JSON, and HTML.
        .filter(|entry| entry.delta > threshold)
        .filter(|entry| wildcard_match(pattern, &entry.name))
        .map(|entry| custom_rule_result(rule, Some(entry.name.clone())))
        .collect()
}

fn evaluate_custom_function_growth(context: &RuleContext<'_>, rule: &CustomRule) -> Vec<RuleResult> {
    let Some(diff) = context.diff else {
        return Vec::new();
    };
    let Some(pattern) = rule.pattern.as_deref() else {
        return Vec::new();
    };
    let threshold = rule.threshold_bytes.or(rule.warn_if_delta_bytes_gt).unwrap_or_default();
    diff.function_diffs
        .iter()
        .filter(|entry| entry.delta > threshold)
        .filter(|entry| wildcard_match(pattern, &entry.name))
        .map(|entry| custom_rule_result(rule, Some(entry.name.clone())))
        .collect()
}

fn evaluate_custom_unknown_source_ratio(context: &RuleContext<'_>, rule: &CustomRule) -> Vec<RuleResult> {
    let Some(threshold) = rule.warn_if_greater_than else {
        return Vec::new();
    };
    let threshold = if threshold <= 1.0 { threshold } else { threshold / 100.0 };
    (context.current.debug_info.unknown_source_ratio >= threshold)
        .then(|| custom_rule_result(rule, Some("unknown_source".to_string())))
        .into_iter()
        .collect()
}

fn custom_rule_result(rule: &CustomRule, related: Option<String>) -> RuleResult {
    RuleResult {
        code: rule.id.clone(),
        severity: match rule.severity {
            RuleSeverityConfig::Info => RuleSeverity::Info,
            RuleSeverityConfig::Warn => RuleSeverity::Warn,
            RuleSeverityConfig::Error => RuleSeverity::Error,
        },
        message: rule.message.clone(),
        related,
    }
}

fn apply_name_filters(name: &str, allowlist: &[String], denylist: &[String]) -> bool {
    let allowed = allowlist.is_empty() || allowlist.iter().any(|item| item == name);
    let denied = denylist.iter().any(|item| item == name);
    allowed && !denied
}

fn normalize_ratio_or_percent(value: f64) -> f64 {
    if value <= 1.0 { value * 100.0 } else { value }
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    // A tiny matcher is enough for current rule files and avoids pulling in a glob engine just for CI thresholds.
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return value.starts_with(prefix);
    }
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        return value.starts_with(prefix) && value.ends_with(suffix);
    }
    value == pattern
}

struct RomUsageHighRule;
struct RamUsageHighRule;
struct UnknownSourceRatioRule;
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

impl Rule for UnknownSourceRatioRule {
    fn evaluate(&self, context: &RuleContext<'_>) -> Vec<RuleResult> {
        (context.current.debug_info.unknown_source_ratio >= context.thresholds.unknown_source_ratio)
            .then(|| RuleResult {
                code: "UNKNOWN_SOURCE_RATIO".to_string(),
                severity: RuleSeverity::Warn,
                message: format!(
                    "Unknown source attribution exceeded {:.0}% ({:.1}%)",
                    context.thresholds.unknown_source_ratio * 100.0,
                    context.current.debug_info.unknown_source_ratio * 100.0
                ),
                related: Some("unknown_source".to_string()),
            })
            .into_iter()
            .collect()
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
        AnalysisResult, BinaryInfo, DebugInfoSummary, DiffChangeKind, DiffEntry, DiffResult, DiffSummary,
        LinkerScriptInfo, MemoryRegion, MemorySummary, RegionUsageSummary, SectionCategory, SectionInfo,
        SectionPlacement, SectionTotal, SymbolInfo, ThresholdConfig, ToolchainInfo, ToolchainKind,
        ToolchainSelection, UnknownSourceBucket, WarningLevel,
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
            unknown_source_delta: 0,
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
            source_file_diffs: Vec::new(),
            function_diffs: Vec::new(),
            line_diffs: Vec::new(),
        };
        let context = RuleContext {
            current: &current,
            diff: Some(&diff),
            thresholds: &ThresholdConfig::default(),
            custom_rules: &[],
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
            unknown_source_delta: 0,
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
        let context = RuleContext {
            current: &current,
            diff: Some(&diff),
            thresholds: &thresholds,
            custom_rules: &[],
        };
        let warnings = evaluate_default_rules(&context);
        assert!(!warnings.iter().any(|warning| warning.code == "SYMBOL_SPIKE"));
        assert!(!warnings.iter().any(|warning| warning.code == "DATA_GROWTH"));
    }

    #[test]
    fn custom_rules_can_raise_error_severity() {
        let current = stub_analysis();
        let custom_rules = vec![crate::model::CustomRule {
            id: "blob-is-forbidden".to_string(),
            kind: crate::model::RuleKind::SymbolMatch,
            severity: crate::model::RuleSeverityConfig::Error,
            message: "blob symbol must not exist".to_string(),
            enabled: true,
            region: None,
            section: None,
            symbol: Some("blob".to_string()),
            object: None,
            pattern: None,
            warn_if_greater_than: None,
            threshold_bytes: None,
            warn_if_delta_bytes_gt: None,
            allowlist: Vec::new(),
            denylist: Vec::new(),
        }];
        let context = RuleContext {
            current: &current,
            diff: None,
            thresholds: &ThresholdConfig::default(),
            custom_rules: &custom_rules,
        };
        let warnings = evaluate_default_rules(&context);
        assert!(warnings.iter().any(|warning| warning.code == "blob-is-forbidden" && warning.level == WarningLevel::Error));
    }

    #[test]
    fn source_growth_and_unknown_source_rules_work() {
        let mut current = stub_analysis();
        current.debug_info.unknown_source_ratio = 0.20;
        let diff = DiffResult {
            rom_delta: 0,
            ram_delta: 0,
            unknown_source_delta: 6,
            summary: DiffSummary::default(),
            section_diffs: Vec::new(),
            symbol_diffs: Vec::new(),
            object_diffs: Vec::new(),
            archive_diffs: Vec::new(),
            source_file_diffs: vec![DiffEntry {
                name: "src/app/main.cpp".to_string(),
                current: 8192,
                previous: 1024,
                delta: 7168,
                change: DiffChangeKind::Increased,
            }],
            function_diffs: vec![DiffEntry {
                name: "src/app/main.cpp::IRQHandler".to_string(),
                current: 2048,
                previous: 256,
                delta: 1792,
                change: DiffChangeKind::Increased,
            }],
            line_diffs: Vec::new(),
        };
        let custom_rules = vec![
            crate::model::CustomRule {
                id: "app_sources_growth".to_string(),
                kind: crate::model::RuleKind::SourcePathGrowth,
                severity: crate::model::RuleSeverityConfig::Warn,
                message: "app sources grew".to_string(),
                enabled: true,
                region: None,
                section: None,
                symbol: None,
                object: None,
                pattern: Some("src/app/**".to_string()),
                warn_if_greater_than: None,
                threshold_bytes: Some(4096),
                warn_if_delta_bytes_gt: None,
                allowlist: Vec::new(),
                denylist: Vec::new(),
            },
            crate::model::CustomRule {
                id: "irq_handler_growth".to_string(),
                kind: crate::model::RuleKind::FunctionGrowth,
                severity: crate::model::RuleSeverityConfig::Error,
                message: "IRQ handler grew".to_string(),
                enabled: true,
                region: None,
                section: None,
                symbol: None,
                object: None,
                pattern: Some("*IRQHandler".to_string()),
                warn_if_greater_than: None,
                threshold_bytes: Some(1024),
                warn_if_delta_bytes_gt: None,
                allowlist: Vec::new(),
                denylist: Vec::new(),
            },
        ];
        let context = RuleContext {
            current: &current,
            diff: Some(&diff),
            thresholds: &ThresholdConfig::default(),
            custom_rules: &custom_rules,
        };
        let warnings = evaluate_default_rules(&context);
        assert!(warnings.iter().any(|warning| warning.code == "UNKNOWN_SOURCE_RATIO"));
        assert!(warnings.iter().any(|warning| warning.code == "app_sources_growth"));
        assert!(warnings.iter().any(|warning| warning.code == "irq_handler_growth" && warning.level == WarningLevel::Error));
    }

    fn stub_analysis() -> AnalysisResult {
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
                linker_family: crate::model::LinkerFamily::Gnu,
                map_format: crate::model::MapFormat::Unknown,
                parser_warnings_count: 0,
            },
            debug_info: DebugInfoSummary::default(),
            debug_artifact: crate::model::DebugArtifactInfo::default(),
            sections: Vec::new(),
            symbols: vec![SymbolInfo {
                name: "blob".to_string(),
                demangled_name: None,
                section_name: None,
                object_path: None,
                addr: 0,
                size: 5000,
            }],
            object_contributions: Vec::new(),
            archive_contributions: Vec::new(),
            archive_pulls: Vec::new(),
            cross_references: Vec::new(),
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
            compilation_units: Vec::new(),
            source_files: Vec::new(),
            line_attributions: Vec::new(),
            line_hotspots: Vec::new(),
            function_attributions: Vec::new(),
            unknown_source: UnknownSourceBucket::default(),
            warnings: Vec::new(),
        }
    }
}
