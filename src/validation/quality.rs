use crate::analyze::format_bytes;
use crate::model::{AnalysisResult, WarningItem, WarningLevel, WarningSource};

pub fn evaluate_quality_checks(current: &AnalysisResult) -> Vec<WarningItem> {
    let mut warnings = Vec::new();

    let region_used_sum = current.memory.region_summaries.iter().map(|item| item.used).sum::<u64>();
    let section_sum = current.sections.iter().map(|item| item.size).sum::<u64>();
    if !current.memory.region_summaries.is_empty() && region_used_sum < section_sum / 2 {
        warnings.push(WarningItem {
            level: WarningLevel::Info,
            code: "REGION_COVERAGE_PARTIAL".to_string(),
            message: format!(
                "Region summaries cover {} while sections total {}; placement data may be partial",
                format_bytes(region_used_sum),
                format_bytes(section_sum)
            ),
            source: WarningSource::Analyze,
            related: None,
        });
    }

    for symbol in &current.symbols {
        if let Some(section_name) = symbol.section_name.as_deref() {
            if !current.sections.iter().any(|section| section.name == section_name) {
                warnings.push(WarningItem {
                    level: WarningLevel::Info,
                    code: "SYMBOL_UNKNOWN_SECTION".to_string(),
                    message: format!("Symbol {} references unknown section {}", symbol.name, section_name),
                    source: WarningSource::Analyze,
                    related: Some(symbol.name.clone()),
                });
            }
        }
    }

    if current.debug_info.split_dwarf_detected && !current.debug_info.dwarf_used {
        warnings.push(WarningItem {
            level: WarningLevel::Info,
            code: "SPLIT_DWARF_DETECTED".to_string(),
            message: "Split DWARF markers were detected but no usable split debug artifact was resolved".to_string(),
            source: WarningSource::Analyze,
            related: current.debug_info.split_dwarf_kind.clone(),
        });
    }

    if current.debug_info.line_zero_ranges > 0 {
        warnings.push(WarningItem {
            level: WarningLevel::Info,
            code: "DWARF_LINE_ZERO_RANGES".to_string(),
            message: format!(
                "{} DWARF ranges used line 0 and were counted as unknown source",
                current.debug_info.line_zero_ranges
            ),
            source: WarningSource::Analyze,
            related: Some("unknown_source".to_string()),
        });
    }

    warnings
}
