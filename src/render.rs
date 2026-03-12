use std::fs;
use std::path::Path;

use crate::analyze::format_bytes;
use crate::demangle::display_name;
use crate::diff::{names_for_kind, top_increases};
use crate::model::{AnalysisResult, DiffChangeKind, DiffEntry, DiffResult, ThresholdConfig, WarningItem};

pub fn print_cli_summary(result: &AnalysisResult, diff: Option<&DiffResult>, verbose: bool) {
    println!("ELF: {}", result.binary.path);
    println!(
        "ROM: {} | RAM: {} | Sections: {} | Symbols: {} | Warnings: {}",
        format_bytes(result.memory.rom_bytes),
        format_bytes(result.memory.ram_bytes),
        result.sections.len(),
        result.symbols.len(),
        result.warnings.len(),
    );
    if let Some(diff) = diff {
        println!("ROM: {:+} bytes", diff.rom_delta);
        println!("RAM: {:+} bytes", diff.ram_delta);
        println!(
            "Diff counts: sections +{} / -{} / ↑{} / ↓{}, symbols +{} / -{} / ↑{} / ↓{}",
            diff.summary.section_added,
            diff.summary.section_removed,
            diff.summary.section_increased,
            diff.summary.section_decreased,
            diff.summary.symbol_added,
            diff.summary.symbol_removed,
            diff.summary.symbol_increased,
            diff.summary.symbol_decreased
        );
    }
    if verbose && !result.warnings.is_empty() {
        println!("Warnings:");
        for item in &result.warnings {
            println!("  [{}:{}] {}", item.source, item.code, item.message);
        }
    }
}

pub fn write_html_report(path: &Path, current: &AnalysisResult, diff: Option<&DiffResult>) -> Result<(), String> {
    let html = build_html(current, diff);
    fs::write(path, html).map_err(|err| format!("failed to write HTML report '{}': {err}", path.display()))
}

pub fn write_json_report(
    path: &Path,
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    thresholds: &ThresholdConfig,
) -> Result<(), String> {
    let json = build_json(current, diff, thresholds)?;
    fs::write(path, json).map_err(|err| format!("failed to write JSON report '{}': {err}", path.display()))
}

pub fn print_ci_summary(current: &AnalysisResult, diff: Option<&DiffResult>) {
    if let Some(diff) = diff {
        println!("ROM: {:+} bytes", diff.rom_delta);
        println!("RAM: {:+} bytes", diff.ram_delta);
        println!("Warnings: {}", current.warnings.len());
        if let Some(entry) = top_increases(&diff.section_diffs, 1).first() {
            println!("Top section growth: {} ({:+})", entry.name, entry.delta);
        }
        if let Some(entry) = top_increases(&diff.symbol_diffs, 1).first() {
            let display = current
                .symbols
                .iter()
                .find(|symbol| symbol.name == entry.name)
                .map(display_name)
                .unwrap_or(&entry.name);
            println!("Top symbol growth: {} ({:+})", display, entry.delta);
        }
    } else {
        println!("ROM: {}", current.memory.rom_bytes);
        println!("RAM: {}", current.memory.ram_bytes);
        println!("Warnings: {}", current.warnings.len());
    }
}

fn build_html(current: &AnalysisResult, diff: Option<&DiffResult>) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>fwmap report</title><style>{}</style></head><body>{}</body></html>",
        style_block(),
        [
            header(current),
            overview(current, diff),
            warning_section(&current.warnings),
            memory_summary(current),
            memory_regions(current),
            region_sections(current),
            section_breakdown(current),
            top_symbols(current),
            top_objects(current),
            diff_section(diff),
            footer(),
        ]
        .join("")
    )
}

fn build_json(current: &AnalysisResult, diff: Option<&DiffResult>, thresholds: &ThresholdConfig) -> Result<String, String> {
    let payload = serde_json::json!({
        "schema_version": 1,
        "binary": &current.binary,
        "linker_script": &current.linker_script,
        "section_summary": &current.memory.section_totals,
        "memory_summary": &current.memory,
        "warnings": &current.warnings,
        "thresholds": thresholds,
        "top_symbols": current.symbols.iter().take(50).collect::<Vec<_>>(),
        "top_object_contributions": current.object_contributions.iter().take(30).collect::<Vec<_>>(),
        "archive_contributions": current.archive_contributions.iter().take(30).collect::<Vec<_>>(),
        "regions": &current.memory.region_summaries,
        "diff_summary": diff.map(|item| &item.summary),
        "diff": diff,
    });
    serde_json::to_string_pretty(&payload).map_err(|err| format!("failed to serialize JSON report: {err}"))
}

fn style_block() -> &'static str {
    "body{font-family:Segoe UI,Arial,sans-serif;margin:24px;background:#f4f1ea;color:#1f2933}h1,h2,h3{margin-bottom:8px}section{background:#fff;padding:16px 18px;border-radius:10px;margin-bottom:16px;box-shadow:0 1px 3px rgba(0,0,0,.08)}table{width:100%;border-collapse:collapse;font-size:14px}th,td{padding:8px;border-bottom:1px solid #d6dde5;text-align:left}th{background:#f0f4f8}.warn{background:#fff3cd}.mono{font-family:Consolas,monospace}.pos{color:#a61b1b}.neg{color:#0a7d33}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px}.card{background:#f8fafc;padding:12px;border-radius:8px}.muted{color:#52606d}"
}

fn header(current: &AnalysisResult) -> String {
    format!(
        "<section><h1>fwmap report</h1><div class=\"muted mono\">{}</div></section>",
        escape(&current.binary.path)
    )
}

fn overview(current: &AnalysisResult, diff: Option<&DiffResult>) -> String {
    let diff_html = diff
        .map(|d| {
            format!(
                "<div class=\"card\"><strong>Diff</strong><div>ROM <span class=\"{}\">{:+}</span></div><div>RAM <span class=\"{}\">{:+}</span></div></div>",
                delta_class(d.rom_delta),
                d.rom_delta,
                delta_class(d.ram_delta),
                d.ram_delta
            )
        })
        .unwrap_or_default();
    format!(
        "<section><h2>Overview</h2><div class=\"grid\"><div class=\"card\"><strong>Binary</strong><div>{}</div></div><div class=\"card\"><strong>Format</strong><div>{} / {}</div></div><div class=\"card\"><strong>Sections</strong><div>{}</div></div><div class=\"card\"><strong>ROM</strong><div>{}</div></div><div class=\"card\"><strong>RAM</strong><div>{}</div></div><div class=\"card\"><strong>Warnings</strong><div>{}</div></div>{}</div></section>",
        escape(&current.binary.arch),
        escape(&current.binary.elf_class),
        escape(&current.binary.endian),
        current.sections.len(),
        format_bytes(current.memory.rom_bytes),
        format_bytes(current.memory.ram_bytes),
        current.warnings.len(),
        diff_html
    )
}

fn warning_section(items: &[WarningItem]) -> String {
    let body = if items.is_empty() {
        "<p>No warnings.</p>".to_string()
    } else {
        let rows = items
            .iter()
            .map(|item| {
                format!(
                    "<tr class=\"warn\"><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    item.level,
                    escape(&item.source.to_string()),
                    escape(&item.code),
                    escape(item.related.as_deref().unwrap_or("-")),
                    escape(&item.message)
                )
            })
            .collect::<Vec<_>>()
            .join("");
        format!("<table><thead><tr><th>Level</th><th>Source</th><th>Code</th><th>Related</th><th>Message</th></tr></thead><tbody>{rows}</tbody></table>")
    };
    format!("<section><h2>Warnings</h2>{body}</section>")
}

fn memory_summary(current: &AnalysisResult) -> String {
    let rows = current
        .memory
        .section_totals
        .iter()
        .take(20)
        .map(|section| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(&section.section_name),
                section.category,
                format_bytes(section.size)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<section><h2>Memory Summary</h2><table><thead><tr><th>Section</th><th>Category</th><th>Size</th></tr></thead><tbody>{rows}</tbody></table></section>")
}

fn memory_regions(current: &AnalysisResult) -> String {
    if current.memory.region_summaries.is_empty() {
        return "<section><h2>Memory Regions Overview</h2><p>No linker script region data was provided.</p></section>".to_string();
    }
    let rows = current
        .memory
        .region_summaries
        .iter()
        .map(|region| {
            format!(
                "<tr><td>{}</td><td class=\"mono\">0x{:x}</td><td>{}</td><td>{}</td><td>{:.1}%<div style=\"background:#d9e2ec;border-radius:999px;height:8px;margin-top:6px;\"><div style=\"width:{:.1}%;background:#c05621;height:8px;border-radius:999px;\"></div></div></td></tr>",
                escape(&region.region_name),
                region.origin,
                format_bytes(region.used),
                format_bytes(region.free),
                region.usage_ratio * 100.0,
                region.usage_ratio * 100.0
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<section><h2>Memory Regions Overview</h2><table><thead><tr><th>Region</th><th>Origin</th><th>Used</th><th>Free</th><th>Usage</th></tr></thead><tbody>{rows}</tbody></table></section>")
}

fn region_sections(current: &AnalysisResult) -> String {
    if current.memory.region_summaries.is_empty() {
        return String::new();
    }
    let blocks = current
        .memory
        .region_summaries
        .iter()
        .map(|region| {
            let rows = if region.sections.is_empty() {
                "<tr><td colspan=\"3\">No mapped sections.</td></tr>".to_string()
            } else {
                region
                    .sections
                    .iter()
                    .map(|section| {
                        format!(
                            "<tr><td>{}</td><td class=\"mono\">0x{:x}</td><td>{}</td></tr>",
                            escape(&section.section_name),
                            section.addr,
                            format_bytes(section.size)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("")
            };
            format!(
                "<h3>{}</h3><table><thead><tr><th>Section</th><th>Address</th><th>Size</th></tr></thead><tbody>{}</tbody></table>",
                escape(&region.region_name),
                rows
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<section><h2>Region Sections</h2>{}</section>", blocks)
}

fn section_breakdown(current: &AnalysisResult) -> String {
    let rows = current
        .sections
        .iter()
        .take(50)
        .map(|section| {
            format!(
                "<tr><td>{}</td><td class=\"mono\">0x{:x}</td><td>{}</td><td>{}</td></tr>",
                escape(&section.name),
                section.addr,
                format_bytes(section.size),
                escape(&section.flags.join(", "))
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<section><h2>Section Breakdown</h2><table><thead><tr><th>Section</th><th>Address</th><th>Size</th><th>Flags</th></tr></thead><tbody>{rows}</tbody></table></section>")
}

fn top_symbols(current: &AnalysisResult) -> String {
    let rows = current
        .symbols
        .iter()
        .take(50)
        .map(|symbol| {
            let display = display_name(symbol);
            let raw = if display != symbol.name {
                format!("<div class=\"muted mono\">{}</div>", escape(&symbol.name))
            } else {
                String::new()
            };
            format!(
                "<tr><td>{}{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(display),
                raw,
                escape(symbol.section_name.as_deref().unwrap_or("-")),
                escape(symbol.object_path.as_deref().unwrap_or("-")),
                format_bytes(symbol.size)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<section><h2>Top Symbols</h2><table><thead><tr><th>Symbol</th><th>Section</th><th>Object</th><th>Size</th></tr></thead><tbody>{rows}</tbody></table></section>")
}

fn top_objects(current: &AnalysisResult) -> String {
    let rows = current
        .object_contributions
        .iter()
        .take(30)
        .map(|item| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(&item.object_path),
                escape(item.section_name.as_deref().unwrap_or("-")),
                format_bytes(item.size)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<section><h2>Top Object Contributions</h2><table><thead><tr><th>Object</th><th>Section</th><th>Size</th></tr></thead><tbody>{rows}</tbody></table></section>")
}

fn diff_section(diff: Option<&DiffResult>) -> String {
    match diff {
        Some(diff) => format!(
            "<section><h2>Diff</h2>{}{}{}{}{}{}{} </section>",
            diff_summary(diff),
            diff_table("Top Section Growth", &top_increases(&diff.section_diffs, 10), 10),
            diff_table("Top Symbol Growth", &top_increases(&diff.symbol_diffs, 10), 10),
            diff_table("Top Object Growth", &top_increases(&diff.object_diffs, 10), 10),
            string_list("Added Symbols", &names_for_kind(&diff.symbol_diffs, DiffChangeKind::Added, 20)),
            string_list("Removed Symbols", &names_for_kind(&diff.symbol_diffs, DiffChangeKind::Removed, 20)),
            string_list("Removed Objects", &names_for_kind(&diff.object_diffs, DiffChangeKind::Removed, 20)),
        ),
        None => "<section><h2>Diff</h2><p>No previous build was provided.</p></section>".to_string(),
    }
}

fn diff_summary(diff: &DiffResult) -> String {
    format!(
        "<div class=\"grid\"><div class=\"card\"><strong>ROM delta</strong><div class=\"{}\">{:+} bytes</div></div><div class=\"card\"><strong>RAM delta</strong><div class=\"{}\">{:+} bytes</div></div><div class=\"card\"><strong>Section changes</strong><div>+{} / -{} / ↑{} / ↓{}</div></div><div class=\"card\"><strong>Symbol changes</strong><div>+{} / -{} / ↑{} / ↓{}</div></div><div class=\"card\"><strong>Object changes</strong><div>+{} / -{} / ↑{} / ↓{}</div></div></div>",
        delta_class(diff.rom_delta),
        diff.rom_delta,
        delta_class(diff.ram_delta),
        diff.ram_delta,
        diff.summary.section_added,
        diff.summary.section_removed,
        diff.summary.section_increased,
        diff.summary.section_decreased,
        diff.summary.symbol_added,
        diff.summary.symbol_removed,
        diff.summary.symbol_increased,
        diff.summary.symbol_decreased,
        diff.summary.object_added,
        diff.summary.object_removed,
        diff.summary.object_increased,
        diff.summary.object_decreased,
    )
}

fn diff_table(title: &str, entries: &[DiffEntry], limit: usize) -> String {
    let rows = entries
        .iter()
        .take(limit)
        .map(|entry| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{:+}</td></tr>",
                escape(&entry.name),
                entry.change,
                format_bytes(entry.current),
                format_bytes(entry.previous),
                delta_class(entry.delta),
                entry.delta
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<h3>{}</h3><table><thead><tr><th>Name</th><th>Change</th><th>Current</th><th>Previous</th><th>Delta</th></tr></thead><tbody>{rows}</tbody></table>", escape(title))
}

fn string_list(title: &str, items: &[String]) -> String {
    let body = if items.is_empty() {
        "<p>-</p>".to_string()
    } else {
        format!("<p>{}</p>", escape(&items.iter().take(20).cloned().collect::<Vec<_>>().join(", ")))
    };
    format!("<h3>{}</h3>{body}", escape(title))
}

fn footer() -> String {
    "<section><h2>Footer</h2><p class=\"muted\">Generated locally by fwmap.</p></section>".to_string()
}

fn delta_class(value: i64) -> &'static str {
    if value > 0 {
        "pos"
    } else if value < 0 {
        "neg"
    } else {
        ""
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::{write_html_report, write_json_report};
    use crate::model::{
        AnalysisResult, BinaryInfo, DiffChangeKind, DiffEntry, DiffResult, DiffSummary, MemoryRegion, MemorySummary,
        RegionSectionUsage, RegionUsageSummary, SectionCategory, SectionInfo, SectionTotal, SymbolInfo, ThresholdConfig,
        WarningItem, WarningLevel, WarningSource,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn html_contains_overview_and_symbols() {
        let path = std::env::temp_dir().join(format!(
            "fwmap-report-{}.html",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let analysis = sample_analysis();
        write_html_report(&path, &analysis, None).unwrap();
        let html = fs::read_to_string(&path).unwrap();
        assert!(html.contains("Overview"));
        assert!(html.contains("Top Symbols"));
        assert!(html.contains("main"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn html_prefers_demangled_symbol_names() {
        let path = std::env::temp_dir().join(format!(
            "fwmap-report-demangle-{}.html",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let mut analysis = sample_analysis();
        analysis.symbols[0].name = "_ZN3foo3barEv".to_string();
        analysis.symbols[0].demangled_name = Some("foo::bar()".to_string());
        write_html_report(&path, &analysis, None).unwrap();
        let html = fs::read_to_string(&path).unwrap();
        assert!(html.contains("foo::bar()"));
        assert!(html.contains("_ZN3foo3barEv"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn html_contains_diff_and_warnings() {
        let path = std::env::temp_dir().join(format!(
            "fwmap-report-diff-{}.html",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let mut analysis = sample_analysis();
        analysis.warnings.push(WarningItem {
            level: WarningLevel::Warn,
            code: "LARGE_SYMBOL".to_string(),
            message: "Large symbol detected".to_string(),
            source: WarningSource::Analyze,
            related: Some("main".to_string()),
        });
        let diff = DiffResult {
            rom_delta: 12,
            ram_delta: -8,
            summary: DiffSummary {
                section_added: 1,
                section_removed: 0,
                section_increased: 1,
                section_decreased: 0,
                symbol_added: 1,
                symbol_removed: 0,
                symbol_increased: 0,
                symbol_decreased: 0,
                object_added: 0,
                object_removed: 0,
                object_increased: 0,
                object_decreased: 0,
            },
            section_diffs: vec![DiffEntry {
                name: ".text".to_string(),
                current: 128,
                previous: 116,
                delta: 12,
                change: DiffChangeKind::Increased,
            }],
            symbol_diffs: vec![DiffEntry {
                name: "main".to_string(),
                current: 64,
                previous: 0,
                delta: 64,
                change: DiffChangeKind::Added,
            }],
            object_diffs: Vec::new(),
            archive_diffs: Vec::new(),
        };
        write_html_report(&path, &analysis, Some(&diff)).unwrap();
        let html = fs::read_to_string(&path).unwrap();
        assert!(html.contains("Warnings"));
        assert!(html.contains("Diff"));
        assert!(html.contains("LARGE_SYMBOL"));
        assert!(html.contains("Added Symbols"));
        assert!(html.contains("Top Symbol Growth"));
        assert!(html.contains("Memory Regions Overview"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn json_contains_thresholds_and_regions() {
        let path = std::env::temp_dir().join(format!(
            "fwmap-report-{}.json",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let analysis = sample_analysis();
        let thresholds = ThresholdConfig::default();
        write_json_report(&path, &analysis, None, &thresholds).unwrap();
        let json = fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"thresholds\""));
        assert!(json.contains("\"regions\""));
        assert!(json.contains("\"demangled_name\""));
        let _ = fs::remove_file(path);
    }

    fn sample_analysis() -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "sample.elf".to_string(),
                arch: "ARM".to_string(),
                elf_class: "ELF32".to_string(),
                endian: "little-endian".to_string(),
            },
            sections: vec![SectionInfo {
                name: ".text".to_string(),
                addr: 0x8000,
                size: 128,
                flags: vec!["ALLOC".to_string(), "EXEC".to_string()],
                category: SectionCategory::Rom,
            }],
            symbols: vec![SymbolInfo {
                name: "main".to_string(),
                demangled_name: None,
                section_name: Some(".text".to_string()),
                object_path: Some("main.o".to_string()),
                size: 64,
            }],
            object_contributions: Vec::new(),
            archive_contributions: Vec::new(),
            linker_script: None,
            memory: MemorySummary {
                rom_bytes: 128,
                ram_bytes: 0,
                section_totals: vec![SectionTotal {
                    section_name: ".text".to_string(),
                    size: 128,
                    category: SectionCategory::Rom,
                }],
                memory_regions: vec![MemoryRegion {
                    name: "FLASH".to_string(),
                    origin: 0x8000,
                    length: 256,
                    attributes: "rx".to_string(),
                }],
                region_summaries: vec![RegionUsageSummary {
                    region_name: "FLASH".to_string(),
                    origin: 0x8000,
                    length: 256,
                    used: 128,
                    free: 128,
                    usage_ratio: 0.5,
                    sections: vec![RegionSectionUsage {
                        section_name: ".text".to_string(),
                        addr: 0x8000,
                        size: 128,
                    }],
                }],
            },
            warnings: Vec::new(),
        }
    }
}
