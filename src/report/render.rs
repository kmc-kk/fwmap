use std::fs;
use std::path::Path;

use crate::analyze::format_bytes;
use crate::demangle::display_name;
use crate::diff::{names_for_kind, top_increases};
use crate::linkage::{explain_top_growth, WhyLinkedCollection};
use crate::model::{
    AnalysisResult, CiFormat, DiffChangeKind, DiffEntry, DiffResult, ObjectSourceKind, ThresholdConfig, WarningItem,
    WarningLevel,
};

#[derive(Debug, Clone, Copy)]
pub struct SourceRenderOptions {
    pub enabled: bool,
    pub max_diff_items: usize,
    pub min_line_diff_bytes: u64,
    pub hide_unknown_source: bool,
}

impl Default for SourceRenderOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            max_diff_items: 10,
            min_line_diff_bytes: 1,
            hide_unknown_source: false,
        }
    }
}

pub fn print_cli_summary(result: &AnalysisResult, diff: Option<&DiffResult>, verbose: bool) {
    println!("ELF: {}", result.binary.path);
    println!("Toolchain: {} (requested: {})", result.toolchain.resolved, result.toolchain.requested);
    println!(
        "Linker family: {} | Map format: {} | Parser warnings: {}",
        result.toolchain.linker_family,
        result.toolchain.map_format,
        result.toolchain.parser_warnings_count
    );
    println!(
        "DWARF: {} | Source lines: {} | Source files: {} | Unknown ratio: {:.1}%{}{}",
        if result.debug_info.dwarf_used { "used" } else { "not used" },
        result.debug_info.source_lines,
        result.source_files.len(),
        result.debug_info.unknown_source_ratio * 100.0,
        if result.debug_info.split_dwarf_detected { " | split-dwarf detected" } else { "" },
        if result.debug_info.cache_hit { " | cache hit" } else { "" }
    );
    if result.debug_artifact.kind != crate::model::DebugArtifactKind::None {
        println!(
            "Debug artifact: {} via {}{}",
            result.debug_artifact.kind,
            result.debug_artifact.source,
            result
                .debug_artifact
                .path
                .as_deref()
                .map(|path| format!(" ({path})"))
                .unwrap_or_default()
        );
    }
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
    if let Some(file) = result.source_files.first() {
        println!("Top source file: {} ({})", file.display_path, format_bytes(file.size));
    }
    if let Some(function) = result.function_attributions.first() {
        println!(
            "Top function: {} ({})",
            function.demangled_name.as_deref().unwrap_or(&function.raw_name),
            format_bytes(function.size)
        );
    }
    if let Some(hotspot) = result.line_hotspots.first() {
        println!(
            "Top line hotspot: {}:{}-{} ({})",
            hotspot.path,
            hotspot.line_start,
            hotspot.line_end,
            format_bytes(hotspot.size)
        );
    }
    if verbose && !result.warnings.is_empty() {
        println!("Warnings:");
        for item in &result.warnings {
            println!("  [{}:{}] {}", item.source, item.code, item.message);
        }
    }
}

pub fn write_html_report(
    path: &Path,
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    source_options: SourceRenderOptions,
    why_linked_top: usize,
) -> Result<(), String> {
    let html = build_html(current, diff, source_options, why_linked_top);
    fs::write(path, html).map_err(|err| format!("failed to write HTML report '{}': {err}", path.display()))
}

pub fn write_json_report(
    path: &Path,
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    thresholds: &ThresholdConfig,
    source_options: SourceRenderOptions,
    why_linked_top: usize,
) -> Result<(), String> {
    let json = build_json(current, diff, thresholds, source_options, why_linked_top)?;
    fs::write(path, json).map_err(|err| format!("failed to write JSON report '{}': {err}", path.display()))
}

pub fn print_ci_summary(
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    format: CiFormat,
    source_options: SourceRenderOptions,
) -> Result<(), String> {
    println!("{}", build_ci_summary(current, diff, format, source_options)?);
    Ok(())
}

pub fn write_ci_summary(
    path: &Path,
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    format: CiFormat,
    source_options: SourceRenderOptions,
) -> Result<(), String> {
    let content = build_ci_summary(current, diff, format, source_options)?;
    fs::write(path, content).map_err(|err| format!("failed to write CI summary '{}': {err}", path.display()))
}

pub fn build_ci_summary(
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    format: CiFormat,
    source_options: SourceRenderOptions,
) -> Result<String, String> {
    match format {
        CiFormat::Text => Ok(build_ci_text(current, diff, source_options)),
        CiFormat::Markdown => Ok(build_ci_markdown(current, diff, source_options)),
        CiFormat::Json => build_ci_json(current, diff, source_options),
    }
}

fn build_html(current: &AnalysisResult, diff: Option<&DiffResult>, source_options: SourceRenderOptions, why_linked_top: usize) -> String {
    let why_linked = diff
        .filter(|_| why_linked_top > 0)
        .map(|item| explain_top_growth(current, item, why_linked_top));
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>fwmap report</title><style>{}</style><script>{}</script></head><body>{}</body></html>",
        style_block(),
        script_block(),
        [
            header(current),
            overview(current, diff),
            warning_section(&current.warnings),
            source_summary(current),
            source_files_section(current),
            top_functions(current),
            line_hotspots(current),
            memory_summary(current),
            memory_regions(current),
            region_sections(current),
            section_breakdown(current),
            top_symbols(current),
            top_objects(current),
            diff_section(current, diff, source_options),
            why_linked_section(why_linked.as_ref()),
            trend_links_section(current),
            footer(),
        ]
        .join("")
    )
}

fn build_json(
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    thresholds: &ThresholdConfig,
    source_options: SourceRenderOptions,
    why_linked_top: usize,
) -> Result<String, String> {
    let why_linked = diff
        .filter(|_| why_linked_top > 0)
        .map(|item| explain_top_growth(current, item, why_linked_top));
    let payload = serde_json::json!({
        "schema_version": 1,
        "binary": &current.binary,
        "toolchain": &current.toolchain,
        "map_format": current.toolchain.map_format,
        "linker_family": current.toolchain.linker_family,
        "debug_info": &current.debug_info,
        "debug_artifact": &current.debug_artifact,
        "linker_script": &current.linker_script,
        "section_summary": &current.memory.section_totals,
        "memory_summary": &current.memory,
        "warnings": &current.warnings,
        "thresholds": thresholds,
        "top_symbols": current.symbols.iter().take(50).collect::<Vec<_>>(),
        "top_object_contributions": current.object_contributions.iter().take(30).collect::<Vec<_>>(),
        "archive_contributions": current.archive_contributions.iter().take(30).collect::<Vec<_>>(),
        "source_files": &current.source_files,
        "functions": &current.function_attributions,
        "line_hotspots": current.line_hotspots.iter().take(100).collect::<Vec<_>>(),
        "line_attributions": current.line_attributions.iter().take(200).collect::<Vec<_>>(),
        "unknown_source": &current.unknown_source,
        "regions": &current.memory.region_summaries,
        "diff_summary": diff.map(|item| &item.summary),
        "diff": diff,
        "source_diff": diff.map(|item| source_diff_payload(item, source_options)),
        "why_linked": why_linked,
    });
    serde_json::to_string_pretty(&payload).map_err(|err| format!("failed to serialize JSON report: {err}"))
}

fn build_ci_text(current: &AnalysisResult, diff: Option<&DiffResult>, source_options: SourceRenderOptions) -> String {
    let mut lines = Vec::new();
    if let Some(diff) = diff {
        lines.push(format!("ROM: {:+} bytes", diff.rom_delta));
        lines.push(format!("RAM: {:+} bytes", diff.ram_delta));
    } else {
        lines.push(format!("ROM: {}", format_bytes(current.memory.rom_bytes)));
        lines.push(format!("RAM: {}", format_bytes(current.memory.ram_bytes)));
    }
    lines.push(format!("Warnings: {}", current.warnings.len()));
    lines.push(format!("Errors: {}", current.warnings.iter().filter(|item| item.level == WarningLevel::Error).count()));
    lines.push(format!("Toolchain: {}", current.toolchain.resolved));
    lines.push(format!(
        "Map: {} / {} (parser warnings: {})",
        current.toolchain.linker_family,
        current.toolchain.map_format,
        current.toolchain.parser_warnings_count
    ));
    lines.push(format!(
        "DWARF: {} (source files: {}, unknown ratio: {:.1}%)",
        if current.debug_info.dwarf_used { "used" } else { "not used" },
        current.source_files.len(),
        current.debug_info.unknown_source_ratio * 100.0
    ));

    if let Some(region) = current.memory.region_summaries.first() {
        lines.push(format!("Top region usage: {} ({:.1}%)", region.region_name, region.usage_ratio * 100.0));
    }
    if let Some(entry) = diff.and_then(|item| top_increases(&item.section_diffs, 1).first().cloned()) {
        lines.push(format!("Top section growth: {} ({:+})", entry.name, entry.delta));
    }
    if let Some(entry) = diff.and_then(|item| top_increases(&item.object_diffs, 1).first().cloned()) {
        lines.push(format!("Top object growth: {} ({:+})", entry.name, entry.delta));
    }
    if let Some(diff) = diff {
        let why = explain_top_growth(current, diff, 1);
        if let Some(item) = why.top_objects.first() {
            lines.push(format!("Why linked object: {}", item.summary));
        } else if let Some(item) = why.top_symbols.first() {
            lines.push(format!("Why linked symbol: {}", item.summary));
        }
    }
    if let Some(entry) = diff.and_then(|item| top_increases(&item.symbol_diffs, 1).first().cloned()) {
        let display = current
            .symbols
            .iter()
            .find(|symbol| symbol.name == entry.name)
            .map(display_name)
            .unwrap_or(&entry.name);
        lines.push(format!("Top symbol growth: {} ({:+})", display, entry.delta));
    }
    if source_options.enabled {
        if let Some(entry) = diff.and_then(|item| top_increases(&item.source_file_diffs, 1).first().cloned()) {
            lines.push(format!("Top source file growth: {} ({:+})", entry.name, entry.delta));
        }
        if let Some(entry) = diff.and_then(|item| top_increases(&item.function_diffs, 1).first().cloned()) {
            lines.push(format!("Top function growth: {} ({:+})", entry.name, entry.delta));
        }
        if let Some(entry) = diff.and_then(|item| {
            filtered_line_diffs(item, source_options)
                .into_iter()
                .next()
        }) {
            lines.push(format!("Top line growth: {} ({:+})", entry.name, entry.delta));
        }
        if let Some(diff) = diff.filter(|_| !source_options.hide_unknown_source) {
            lines.push(format!("Unknown source delta: {:+} bytes", diff.unknown_source_delta));
        }
    }
    if !current.warnings.is_empty() {
        let triggered = current
            .warnings
            .iter()
            .map(|item| format!("{}({})", item.code, item.level))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("Triggered rules: {triggered}"));
    }
    lines.join("\n")
}

fn why_linked_section(items: Option<&WhyLinkedCollection>) -> String {
    let Some(items) = items else {
        return String::new();
    };
    if items.top_symbols.is_empty() && items.top_objects.is_empty() {
        return String::new();
    }
    let symbol_rows = items
        .top_symbols
        .iter()
        .map(|item| format!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>", escape(&item.target), item.confidence.to_string(), escape(&item.summary)))
        .collect::<Vec<_>>()
        .join("");
    let object_rows = items
        .top_objects
        .iter()
        .map(|item| format!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>", escape(&item.target), item.confidence.to_string(), escape(&item.summary)))
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<section id=\"why-linked\"><h2>Why Linked</h2><div class=\"grid\"><div><h3>Top Symbols</h3><table><thead><tr><th>Target</th><th>Confidence</th><th>Summary</th></tr></thead><tbody>{}</tbody></table></div><div><h3>Top Objects</h3><table><thead><tr><th>Target</th><th>Confidence</th><th>Summary</th></tr></thead><tbody>{}</tbody></table></div></div></section>",
        symbol_rows,
        object_rows
    )
}

fn build_ci_markdown(current: &AnalysisResult, diff: Option<&DiffResult>, source_options: SourceRenderOptions) -> String {
    let mut out = Vec::new();
    out.push("# fwmap CI Summary".to_string());
    out.push(String::new());
    out.push("| Metric | Value |".to_string());
    out.push("| --- | --- |".to_string());
    if let Some(diff) = diff {
        out.push(format!("| ROM delta | {:+} bytes |", diff.rom_delta));
        out.push(format!("| RAM delta | {:+} bytes |", diff.ram_delta));
    } else {
        out.push(format!("| ROM | {} |", format_bytes(current.memory.rom_bytes)));
        out.push(format!("| RAM | {} |", format_bytes(current.memory.ram_bytes)));
    }
    out.push(format!("| Warnings | {} |", current.warnings.len()));
    out.push(format!(
        "| Errors | {} |",
        current.warnings.iter().filter(|item| item.level == WarningLevel::Error).count()
    ));
    out.push(format!("| Toolchain | {} |", current.toolchain.resolved));
    out.push(format!(
        "| Map | {} / {} (warnings: {}) |",
        current.toolchain.linker_family,
        current.toolchain.map_format,
        current.toolchain.parser_warnings_count
    ));
    out.push(format!(
        "| DWARF | {} ({:.1}% unknown) |",
        if current.debug_info.dwarf_used { "used" } else { "not used" },
        current.debug_info.unknown_source_ratio * 100.0
    ));
    out.push(String::new());

    let mut growths = Vec::new();
    if let Some(entry) = diff.and_then(|item| top_increases(&item.section_diffs, 1).first().cloned()) {
        growths.push(format!("- Top section growth: `{}` ({:+})", entry.name, entry.delta));
    }
    if let Some(entry) = diff.and_then(|item| top_increases(&item.object_diffs, 1).first().cloned()) {
        growths.push(format!("- Top object growth: `{}` ({:+})", entry.name, entry.delta));
    }
    if let Some(entry) = diff.and_then(|item| top_increases(&item.symbol_diffs, 1).first().cloned()) {
        let display = current
            .symbols
            .iter()
            .find(|symbol| symbol.name == entry.name)
            .map(display_name)
            .unwrap_or(&entry.name);
        growths.push(format!("- Top symbol growth: `{}` ({:+})", display, entry.delta));
    }
    if !growths.is_empty() {
        out.push("## Growth".to_string());
        out.extend(growths);
        out.push(String::new());
    }

    if let Some(diff) = diff {
        let why = explain_top_growth(current, diff, 1);
        let mut explain = Vec::new();
        if let Some(item) = why.top_symbols.first() {
            explain.push(format!("- Symbol: {} ({})", item.summary, item.confidence));
        }
        if let Some(item) = why.top_objects.first() {
            explain.push(format!("- Object: {} ({})", item.summary, item.confidence));
        }
        if !explain.is_empty() {
            out.push("## Why Linked".to_string());
            out.extend(explain);
            out.push(String::new());
        }
    }

    if source_options.enabled {
        let mut source = Vec::new();
        if let Some(entry) = diff.and_then(|item| top_increases(&item.source_file_diffs, 1).first().cloned()) {
            source.push(format!("- Top source file growth: `{}` ({:+})", entry.name, entry.delta));
        }
        if let Some(entry) = diff.and_then(|item| top_increases(&item.function_diffs, 1).first().cloned()) {
            source.push(format!("- Top function growth: `{}` ({:+})", entry.name, entry.delta));
        }
        if let Some(entry) = diff.and_then(|item| filtered_line_diffs(item, source_options).into_iter().next()) {
            source.push(format!("- Top line growth: `{}` ({:+})", entry.name, entry.delta));
        }
        if let Some(diff) = diff.filter(|_| !source_options.hide_unknown_source) {
            source.push(format!("- Unknown source delta: `{:+}` bytes", diff.unknown_source_delta));
        }
        if !source.is_empty() {
            out.push("## Source Diff".to_string());
            out.extend(source);
            out.push(String::new());
        }
    }

    out.push("## Rule Results".to_string());
    if current.warnings.is_empty() {
        out.push("- No warnings.".to_string());
    } else {
        out.extend(current.warnings.iter().map(|item| {
            format!(
                "- `{}` [{}] {}",
                item.code,
                item.level,
                item.related.as_deref().unwrap_or(&item.message)
            )
        }));
    }
    out.join("\n")
}

fn build_ci_json(
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    source_options: SourceRenderOptions,
) -> Result<String, String> {
    let payload = serde_json::json!({
        "schema_version": 1,
        "summary": {
            "rom_bytes": current.memory.rom_bytes,
            "ram_bytes": current.memory.ram_bytes,
            "rom_delta": diff.map(|item| item.rom_delta),
            "ram_delta": diff.map(|item| item.ram_delta),
            "warning_count": current.warnings.len(),
            "error_count": current.warnings.iter().filter(|item| item.level == WarningLevel::Error).count(),
            "toolchain": current.toolchain.resolved,
            "linker_family": current.toolchain.linker_family,
            "map_format": current.toolchain.map_format,
            "parser_warnings_count": current.toolchain.parser_warnings_count,
            "dwarf_used": current.debug_info.dwarf_used,
            "unknown_source_ratio": current.debug_info.unknown_source_ratio,
        },
        "debug_info": &current.debug_info,
        "source_files": &current.source_files,
        "functions": &current.function_attributions,
        "line_hotspots": current.line_hotspots.iter().take(20).collect::<Vec<_>>(),
        "top_region": current.memory.region_summaries.first(),
        "top_section_growth": diff.and_then(|item| top_increases(&item.section_diffs, 1).first().cloned()),
        "top_object_growth": diff.and_then(|item| top_increases(&item.object_diffs, 1).first().cloned()),
        "top_symbol_growth": diff.and_then(|item| top_increases(&item.symbol_diffs, 1).first().cloned()),
        "why_linked": diff.map(|item| explain_top_growth(current, item, 1)),
        "top_source_file_growth": diff.and_then(|item| top_increases(&item.source_file_diffs, 1).first().cloned()),
        "top_function_growth": diff.and_then(|item| top_increases(&item.function_diffs, 1).first().cloned()),
        "top_line_growth": diff.and_then(|item| filtered_line_diffs(item, source_options).into_iter().next()),
        "unknown_source_delta": diff
            .filter(|_| !source_options.hide_unknown_source)
            .map(|item| item.unknown_source_delta),
        "rules": &current.warnings,
    });
    serde_json::to_string_pretty(&payload).map_err(|err| format!("failed to serialize CI JSON summary: {err}"))
}

fn style_block() -> &'static str {
    "body{font-family:Segoe UI,Arial,sans-serif;margin:24px;background:#f4f1ea;color:#1f2933}h1,h2,h3{margin-bottom:8px}section{background:#fff;padding:16px 18px;border-radius:10px;margin-bottom:16px;box-shadow:0 1px 3px rgba(0,0,0,.08)}table{width:100%;border-collapse:collapse;font-size:14px}th,td{padding:8px;border-bottom:1px solid #d6dde5;text-align:left;vertical-align:top}th{background:#f0f4f8}.warn{background:#fff3cd}.mono{font-family:Consolas,monospace}.pos{color:#a61b1b}.neg{color:#0a7d33}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:12px}.card{background:#f8fafc;padding:12px;border-radius:8px}.muted{color:#52606d}.toolbar{display:flex;flex-wrap:wrap;gap:8px;margin:10px 0 12px}.toolbar input{padding:8px 10px;border:1px solid #cbd2d9;border-radius:8px;min-width:180px;background:#fff}.pill{display:inline-block;padding:2px 8px;border-radius:999px;background:#e9eff5;font-size:12px;color:#334e68}.path{display:inline-block;max-width:32rem;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.anchor{color:inherit;text-decoration:none}.anchor:hover{text-decoration:underline}.hidden{display:none}.hint{margin-top:8px;font-size:13px}"
}

fn script_block() -> &'static str {
    "document.addEventListener('DOMContentLoaded',()=>{const apply=(target)=>{const table=document.getElementById(target);if(!table)return;const controls=[...document.querySelectorAll(`[data-filter-target=\"${target}\"]`)];const rows=[...table.querySelectorAll('tbody tr')];rows.forEach((row)=>{const visible=controls.every((control)=>{const key=control.dataset.filterKey;const value=control.value.trim().toLowerCase();if(!value)return true;return String(row.dataset[key]||'').toLowerCase().includes(value);});row.classList.toggle('hidden',!visible);});};document.querySelectorAll('[data-filter-target]').forEach((control)=>{control.addEventListener('input',()=>apply(control.dataset.filterTarget));apply(control.dataset.filterTarget);});});"
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
        "<section><h2>Overview</h2><div class=\"grid\"><div class=\"card\"><strong>Binary</strong><div>{}</div></div><div class=\"card\"><strong>Format</strong><div>{} / {}</div></div><div class=\"card\"><strong>Toolchain</strong><div>{} <span class=\"muted\">(requested: {})</span></div></div><div class=\"card\"><strong>Map</strong><div>{} / {} <span class=\"muted\">({} parser warnings)</span></div></div><div class=\"card\"><strong>DWARF</strong><div>{} <span class=\"muted\">({:.1}% unknown)</span></div></div><div class=\"card\"><strong>Debug Artifact</strong><div>{}</div></div><div class=\"card\"><strong>Sections</strong><div>{}</div></div><div class=\"card\"><strong>ROM</strong><div>{}</div></div><div class=\"card\"><strong>RAM</strong><div>{}</div></div><div class=\"card\"><strong>Warnings</strong><div>{}</div></div>{}</div></section>",
        escape(&current.binary.arch),
        escape(&current.binary.elf_class),
        escape(&current.binary.endian),
        escape(&current.toolchain.resolved.to_string()),
        escape(&current.toolchain.requested.to_string()),
        escape(&current.toolchain.linker_family.to_string()),
        escape(&current.toolchain.map_format.to_string()),
        current.toolchain.parser_warnings_count,
        if current.debug_info.dwarf_used { "used" } else { "not used" },
        current.debug_info.unknown_source_ratio * 100.0,
        escape(&debug_artifact_summary(current)),
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

fn source_summary(current: &AnalysisResult) -> String {
    if !current.debug_info.dwarf_used {
        return "<section><h2>Source Summary</h2><p>No DWARF line information was used.</p></section>".to_string();
    }
    let rows = current
        .source_files
        .iter()
        .take(20)
        .map(|item| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(&item.display_path),
                item.line_ranges,
                format_bytes(item.size)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<section id=\"source-summary\"><h2>Source Summary</h2><p>Compilation units: {} | Unknown ratio: {:.1}% | Line-0 ranges: {}{}</p><p class=\"hint\">Use the trend links below to jump to ready-made history metrics for source files, functions, and unknown source ratio.</p><table><thead><tr><th>Path</th><th>Line Ranges</th><th>Size</th></tr></thead><tbody>{}</tbody></table></section>",
        current.debug_info.compilation_units,
        current.debug_info.unknown_source_ratio * 100.0,
        current.debug_info.line_zero_ranges,
        current
            .debug_info
            .split_dwarf_kind
            .as_ref()
            .map(|kind| format!(" | Split DWARF: {}", escape(kind)))
            .unwrap_or_default(),
        rows
    )
}

fn source_files_section(current: &AnalysisResult) -> String {
    if !current.debug_info.dwarf_used || current.source_files.is_empty() {
        return String::new();
    }
    let rows = current
        .source_files
        .iter()
        .take(30)
        .map(|item| {
            let trend_id = trend_anchor_id("source", &item.path);
            format!(
                "<tr data-search=\"{} {}\" data-path=\"{}\"><td title=\"{}\"><a class=\"anchor path\" href=\"#{}\">{}</a></td><td><span class=\"path\" title=\"{}\">{}</span></td><td>{}</td><td>{}</td></tr>",
                escape(&item.path),
                escape(&item.directory),
                escape(&item.path),
                escape(&item.path),
                trend_id,
                escape(&short_path(&item.display_path)),
                escape(&item.directory),
                escape(&short_path(&item.directory)),
                item.functions,
                format_bytes(item.size)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<section id=\"source-files\"><h2>Source Files</h2><div class=\"toolbar\"><input type=\"search\" placeholder=\"Search files\" data-filter-target=\"source-files-table\" data-filter-key=\"search\"><input type=\"search\" placeholder=\"Filter path\" data-filter-target=\"source-files-table\" data-filter-key=\"path\"></div><table id=\"source-files-table\"><thead><tr><th>Path</th><th>Directory</th><th>Functions</th><th>Size</th></tr></thead><tbody>{rows}</tbody></table></section>"
    )
}

fn top_functions(current: &AnalysisResult) -> String {
    if !current.debug_info.dwarf_used || current.function_attributions.is_empty() {
        return String::new();
    }
    let rows = current
        .function_attributions
        .iter()
        .take(30)
        .map(|item| {
            let name = item.demangled_name.as_deref().unwrap_or(&item.raw_name);
            let raw = if item.demangled_name.is_some() {
                format!("<div class=\"muted mono\">{}</div>", escape(&item.raw_name))
            } else {
                String::new()
            };
            let ranges = item
                .ranges
                .iter()
                .take(3)
                .map(|range| {
                    let anchor = line_anchor_id(&range.path, range.line_start, range.line_end);
                    format!(
                        "<a class=\"anchor\" href=\"#{}\">{}:{}-{}</a>",
                        anchor,
                        escape(&short_path(&range.path)),
                        range.line_start,
                        range.line_end
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let path = item.path.as_deref().unwrap_or("-");
            let trend_id = trend_anchor_id("function", &function_metric_key(item.path.as_deref(), &item.raw_name));
            format!(
                "<tr data-search=\"{} {} {}\" data-path=\"{}\"><td><a class=\"anchor\" href=\"#{}\">{}</a>{}</td><td><span class=\"path\" title=\"{}\">{}</span></td><td>{}</td><td>{}</td></tr>",
                escape(name),
                escape(&item.raw_name),
                escape(path),
                escape(path),
                trend_id,
                escape(name),
                raw,
                escape(path),
                escape(&short_path(path)),
                ranges,
                format_bytes(item.size)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<section id=\"top-functions\"><h2>Top Functions</h2><div class=\"toolbar\"><input type=\"search\" placeholder=\"Search functions\" data-filter-target=\"top-functions-table\" data-filter-key=\"search\"><input type=\"search\" placeholder=\"Filter path\" data-filter-target=\"top-functions-table\" data-filter-key=\"path\"></div><table id=\"top-functions-table\"><thead><tr><th>Function</th><th>Path</th><th>Ranges</th><th>Size</th></tr></thead><tbody>{rows}</tbody></table></section>"
    )
}

fn line_hotspots(current: &AnalysisResult) -> String {
    if !current.debug_info.dwarf_used || current.line_hotspots.is_empty() {
        return String::new();
    }
    let rows = current
        .line_hotspots
        .iter()
        .take(30)
        .map(|item| {
            let anchor = line_anchor_id(&item.path, item.line_start, item.line_end);
            format!(
                "<tr id=\"{}\" data-search=\"{} {}\" data-path=\"{}\" data-section=\"{}\"><td title=\"{}\"><a class=\"anchor path\" href=\"#{}\">{}</a></td><td>{}-{} </td><td>{}</td><td>{}</td></tr>",
                anchor,
                escape(&item.path),
                escape(item.section_name.as_deref().unwrap_or("-")),
                escape(&item.path),
                escape(item.section_name.as_deref().unwrap_or("-")),
                escape(&item.path),
                anchor,
                escape(&short_path(&item.path)),
                item.line_start,
                item.line_end,
                escape(item.section_name.as_deref().unwrap_or("-")),
                format_bytes(item.size)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<section id=\"line-hotspots\"><h2>Line Hotspots</h2><div class=\"toolbar\"><input type=\"search\" placeholder=\"Search lines\" data-filter-target=\"line-hotspots-table\" data-filter-key=\"search\"><input type=\"search\" placeholder=\"Filter path\" data-filter-target=\"line-hotspots-table\" data-filter-key=\"path\"><input type=\"search\" placeholder=\"Filter section\" data-filter-target=\"line-hotspots-table\" data-filter-key=\"section\"></div><table id=\"line-hotspots-table\"><thead><tr><th>Path</th><th>Lines</th><th>Section</th><th>Size</th></tr></thead><tbody>{rows}</tbody></table></section>"
    )
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
                "<tr data-search=\"{}\" data-region=\"{}\"><td>{}</td><td class=\"mono\">0x{:x}</td><td>{}</td><td>{}</td><td>{:.1}%<div style=\"background:#d9e2ec;border-radius:999px;height:8px;margin-top:6px;\"><div style=\"width:{:.1}%;background:#c05621;height:8px;border-radius:999px;\"></div></div></td></tr>",
                escape(&region.region_name),
                escape(&region.region_name),
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
    format!("<section id=\"memory-regions\"><h2>Memory Regions Overview</h2><div class=\"toolbar\"><input type=\"search\" placeholder=\"Filter region\" data-filter-target=\"memory-regions-table\" data-filter-key=\"region\"></div><table id=\"memory-regions-table\"><thead><tr><th>Region</th><th>Origin</th><th>Used</th><th>Free</th><th>Usage</th></tr></thead><tbody>{rows}</tbody></table></section>")
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
                            "<tr data-search=\"{} {}\" data-region=\"{}\" data-section=\"{}\"><td>{}</td><td class=\"mono\">0x{:x}</td><td>{}</td></tr>",
                            escape(&region.region_name),
                            escape(&section.section_name),
                            escape(&region.region_name),
                            escape(&section.section_name),
                            escape(&section.section_name),
                            section.addr,
                            format_bytes(section.size)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("")
            };
            format!(
                "<h3>{}</h3><table id=\"region-sections-{}\"><thead><tr><th>Section</th><th>Address</th><th>Size</th></tr></thead><tbody>{}</tbody></table>",
                escape(&region.region_name),
                slugify(&region.region_name),
                rows
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<section id=\"region-sections\"><h2>Region Sections</h2><div class=\"toolbar\"><input type=\"search\" placeholder=\"Filter region or section\" data-filter-target=\"region-sections-all\" data-filter-key=\"search\"></div><div id=\"region-sections-all\">{}</div></section>", blocks)
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
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape(&item.object_path),
                escape(object_source_kind_label(item.source_kind)),
                escape(item.section_name.as_deref().unwrap_or("-")),
                format_bytes(item.size)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<section><h2>Top Object Contributions</h2><table><thead><tr><th>Object</th><th>Kind</th><th>Section</th><th>Size</th></tr></thead><tbody>{rows}</tbody></table></section>"
    )
}

fn object_source_kind_label(kind: ObjectSourceKind) -> &'static str {
    match kind {
        ObjectSourceKind::Object => "object",
        ObjectSourceKind::Internal => "internal",
    }
}

fn diff_section(current: &AnalysisResult, diff: Option<&DiffResult>, source_options: SourceRenderOptions) -> String {
    match diff {
        Some(diff) => format!(
            "<section><h2>Diff</h2>{}{}{}{}{}{}{}{} </section>",
            diff_summary(diff),
            diff_table("Top Section Growth", &top_increases(&diff.section_diffs, 10), 10),
            diff_table("Top Symbol Growth", &top_increases(&diff.symbol_diffs, 10), 10),
            diff_table("Top Object Growth", &top_increases(&diff.object_diffs, 10), 10),
            source_diff_section(current, diff, source_options),
            string_list("Added Symbols", &names_for_kind(&diff.symbol_diffs, DiffChangeKind::Added, 20)),
            string_list("Removed Symbols", &names_for_kind(&diff.symbol_diffs, DiffChangeKind::Removed, 20)),
            string_list("Removed Objects", &names_for_kind(&diff.object_diffs, DiffChangeKind::Removed, 20)),
        ),
        None => "<section><h2>Diff</h2><p>No previous build was provided.</p></section>".to_string(),
    }
}

fn diff_summary(diff: &DiffResult) -> String {
    format!(
        "<div class=\"grid\"><div class=\"card\"><strong>ROM delta</strong><div class=\"{}\">{:+} bytes</div></div><div class=\"card\"><strong>RAM delta</strong><div class=\"{}\">{:+} bytes</div></div><div class=\"card\"><strong>Unknown source</strong><div class=\"{}\">{:+} bytes</div></div><div class=\"card\"><strong>Section changes</strong><div>+{} / -{} / ↑{} / ↓{}</div></div><div class=\"card\"><strong>Symbol changes</strong><div>+{} / -{} / ↑{} / ↓{}</div></div><div class=\"card\"><strong>Object changes</strong><div>+{} / -{} / ↑{} / ↓{}</div></div></div>",
        delta_class(diff.rom_delta),
        diff.rom_delta,
        delta_class(diff.ram_delta),
        diff.ram_delta,
        delta_class(diff.unknown_source_delta),
        diff.unknown_source_delta,
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

fn source_diff_section(current: &AnalysisResult, diff: &DiffResult, source_options: SourceRenderOptions) -> String {
    let limit = source_options.max_diff_items.max(1);
    let file_block = diff_table("Top Source File Growth", &top_increases(&diff.source_file_diffs, limit), limit);
    let function_entries = top_increases(&diff.function_diffs, limit)
        .into_iter()
        .map(|mut entry| {
            entry.name = function_display_name(current, &entry.name);
            entry
        })
        .collect::<Vec<_>>();
    let function_block = diff_table("Top Function Growth", &function_entries, limit);
    let line_block = diff_table("Top Line Growth", &filtered_line_diffs(diff, source_options), limit);
    let unknown_block = if source_options.hide_unknown_source {
        String::new()
    } else {
        format!("<h3>Unknown Source</h3><p>{:+} bytes</p>", diff.unknown_source_delta)
    };
    format!("<h3 id=\"diff-source\">Source Diff</h3>{file_block}{function_block}{line_block}{unknown_block}")
}

fn filtered_line_diffs(diff: &DiffResult, source_options: SourceRenderOptions) -> Vec<DiffEntry> {
    diff.line_diffs
        .iter()
        .filter(|entry| entry.delta.unsigned_abs() >= source_options.min_line_diff_bytes)
        .take(source_options.max_diff_items.max(1))
        .cloned()
        .collect()
}

fn source_diff_payload(diff: &DiffResult, source_options: SourceRenderOptions) -> serde_json::Value {
    serde_json::json!({
        "unknown_source_delta": if source_options.hide_unknown_source {
            serde_json::Value::Null
        } else {
            serde_json::json!(diff.unknown_source_delta)
        },
        "source_files": top_increases(&diff.source_file_diffs, source_options.max_diff_items.max(1)),
        "functions": top_increases(&diff.function_diffs, source_options.max_diff_items.max(1)),
        "lines": filtered_line_diffs(diff, source_options),
    })
}

fn function_display_name(current: &AnalysisResult, key: &str) -> String {
    current
        .function_attributions
        .iter()
        .find_map(|item| {
            let item_key = match item.path.as_deref() {
                Some(path) => format!("{path}::{}", item.raw_name),
                None => item.raw_name.clone(),
            };
            (item_key == key).then(|| match item.demangled_name.as_deref() {
                Some(demangled) => format!("{demangled} [{key}]"),
                None => key.to_string(),
            })
        })
        .unwrap_or_else(|| key.to_string())
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

fn debug_artifact_summary(current: &AnalysisResult) -> String {
    if current.debug_artifact.kind == crate::model::DebugArtifactKind::None {
        return "not found".to_string();
    }
    let mut text = format!("{} via {}", current.debug_artifact.kind, current.debug_artifact.source);
    if let Some(path) = current.debug_artifact.path.as_deref() {
        text.push_str(&format!(" ({})", short_path(path)));
    }
    text
}

fn trend_links_section(current: &AnalysisResult) -> String {
    if !current.debug_info.dwarf_used {
        return String::new();
    }
    let mut blocks = Vec::new();
    blocks.push(format!(
        "<div id=\"{}\"><h3>Unknown Source Ratio</h3><p class=\"mono\">fwmap history trend --db history.db --metric unknown_source --last 20</p></div>",
        trend_anchor_id("debug", "unknown_source")
    ));
    for source in current.source_files.iter().take(5) {
        blocks.push(format!(
            "<div id=\"{}\"><h3>Source Trend: {}</h3><p class=\"mono\">fwmap history trend --db history.db --metric \"source:{}\" --last 20</p></div>",
            trend_anchor_id("source", &source.path),
            escape(&short_path(&source.display_path)),
            escape(&source.path)
        ));
        blocks.push(format!(
            "<div id=\"{}\"><h3>Directory Trend: {}</h3><p class=\"mono\">fwmap history trend --db history.db --metric \"directory:{}\" --last 20</p></div>",
            trend_anchor_id("directory", &source.directory),
            escape(&short_path(&source.directory)),
            escape(&source.directory)
        ));
    }
    for function in current.function_attributions.iter().take(5) {
        let key = function_metric_key(function.path.as_deref(), &function.raw_name);
        let name = function.demangled_name.as_deref().unwrap_or(&function.raw_name);
        blocks.push(format!(
            "<div id=\"{}\"><h3>Function Trend: {}</h3><p class=\"mono\">fwmap history trend --db history.db --metric \"function:{}\" --last 20</p></div>",
            trend_anchor_id("function", &key),
            escape(name),
            escape(&key)
        ));
    }
    format!("<section id=\"trend-links\"><h2>Trend Links</h2>{}</section>", blocks.join(""))
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

fn short_path(path: &str) -> String {
    let separators = ['/', '\\'];
    let parts = path.split(separators).collect::<Vec<_>>();
    if parts.len() <= 3 {
        return path.to_string();
    }
    format!("{}/.../{}", parts[0], parts[parts.len() - 1])
}

fn slugify(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
}

fn line_anchor_id(path: &str, line_start: u64, line_end: u64) -> String {
    format!("line-{}-{}-{}", slugify(path), line_start, line_end)
}

fn trend_anchor_id(kind: &str, key: &str) -> String {
    format!("trend-{}-{}", kind, slugify(key))
}

fn function_metric_key(path: Option<&str>, raw_name: &str) -> String {
    match path {
        Some(path) => format!("{path}::{raw_name}"),
        None => raw_name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{build_ci_summary, write_html_report, write_json_report, SourceRenderOptions};
    use crate::model::{
        AnalysisResult, BinaryInfo, CiFormat, DebugArtifactInfo, DebugInfoSummary, DiffChangeKind, DiffEntry,
        DiffResult, DiffSummary, MemoryRegion, MemorySummary, RegionSectionUsage, RegionUsageSummary, SectionCategory,
        SectionInfo, SectionTotal, SymbolInfo, ThresholdConfig, ToolchainInfo, ToolchainKind, ToolchainSelection,
        UnknownSourceBucket, WarningItem, WarningLevel, WarningSource,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn html_contains_overview_and_symbols() {
        let path = std::env::temp_dir().join(format!(
            "fwmap-report-{}.html",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let analysis = sample_analysis_with_sources();
        write_html_report(&path, &analysis, None, SourceRenderOptions::default(), 3).unwrap();
        let html = fs::read_to_string(&path).unwrap();
        assert!(html.contains("Overview"));
        assert!(html.contains("Top Symbols"));
        assert!(html.contains("Source Files"));
        assert!(html.contains("Top Functions"));
        assert!(html.contains("Line Hotspots"));
        assert!(html.contains("Search files"));
        assert!(html.contains("Trend Links"));
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
        write_html_report(&path, &analysis, None, SourceRenderOptions::default(), 3).unwrap();
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
        let mut analysis = sample_analysis_with_sources();
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
            unknown_source_delta: 4,
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
                source_file_added: 0,
                source_file_removed: 0,
                source_file_increased: 1,
                source_file_decreased: 0,
                function_added: 0,
                function_removed: 0,
                function_increased: 1,
                function_decreased: 0,
                line_added: 0,
                line_removed: 0,
                line_increased: 1,
                line_decreased: 0,
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
            source_file_diffs: vec![DiffEntry {
                name: "src/main.cpp".to_string(),
                current: 64,
                previous: 32,
                delta: 32,
                change: DiffChangeKind::Increased,
            }],
            function_diffs: vec![DiffEntry {
                name: "src/main.cpp::_ZN3app4mainEv".to_string(),
                current: 64,
                previous: 32,
                delta: 32,
                change: DiffChangeKind::Increased,
            }],
            line_diffs: vec![DiffEntry {
                name: "src/main.cpp:10-12".to_string(),
                current: 64,
                previous: 32,
                delta: 32,
                change: DiffChangeKind::Increased,
            }],
        };
        write_html_report(
            &path,
            &analysis,
            Some(&diff),
            SourceRenderOptions {
                enabled: true,
                ..SourceRenderOptions::default()
            },
            3,
        )
        .unwrap();
        let html = fs::read_to_string(&path).unwrap();
        assert!(html.contains("Warnings"));
        assert!(html.contains("Diff"));
        assert!(html.contains("LARGE_SYMBOL"));
        assert!(html.contains("Added Symbols"));
        assert!(html.contains("Top Symbol Growth"));
        assert!(html.contains("Source Diff"));
        assert!(html.contains("Memory Regions Overview"));
        assert!(html.contains("Filter section"));
        assert!(html.contains("Unknown Source Ratio"));
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
        write_json_report(&path, &analysis, None, &thresholds, SourceRenderOptions::default(), 3).unwrap();
        let json = fs::read_to_string(&path).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"thresholds\""));
        assert!(json.contains("\"regions\""));
        assert!(json.contains("\"demangled_name\""));
        assert!(json.contains("\"toolchain\""));
        assert!(json.contains("\"debug_info\""));
        assert!(json.contains("\"source_files\""));
        assert!(json.contains("\"line_hotspots\""));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn ci_summary_supports_text_markdown_and_json() {
        let mut analysis = sample_analysis_with_sources();
        analysis.warnings.push(WarningItem {
            level: WarningLevel::Error,
            code: "forbid-main".to_string(),
            message: "main symbol is forbidden".to_string(),
            source: WarningSource::Analyze,
            related: Some("main".to_string()),
        });
        let diff = DiffResult {
            rom_delta: 16,
            ram_delta: 4,
            unknown_source_delta: 8,
            summary: DiffSummary::default(),
            section_diffs: vec![DiffEntry {
                name: ".text".to_string(),
                current: 128,
                previous: 112,
                delta: 16,
                change: DiffChangeKind::Increased,
            }],
            symbol_diffs: vec![DiffEntry {
                name: "main".to_string(),
                current: 64,
                previous: 48,
                delta: 16,
                change: DiffChangeKind::Increased,
            }],
            object_diffs: vec![DiffEntry {
                name: "main.o".to_string(),
                current: 64,
                previous: 48,
                delta: 16,
                change: DiffChangeKind::Increased,
            }],
            archive_diffs: Vec::new(),
            source_file_diffs: vec![DiffEntry {
                name: "src/main.cpp".to_string(),
                current: 64,
                previous: 48,
                delta: 16,
                change: DiffChangeKind::Increased,
            }],
            function_diffs: vec![DiffEntry {
                name: "src/main.cpp::_ZN3app4mainEv".to_string(),
                current: 64,
                previous: 48,
                delta: 16,
                change: DiffChangeKind::Increased,
            }],
            line_diffs: vec![DiffEntry {
                name: "src/main.cpp:10-12".to_string(),
                current: 64,
                previous: 48,
                delta: 16,
                change: DiffChangeKind::Increased,
            }],
        };
        let text = build_ci_summary(
            &analysis,
            Some(&diff),
            CiFormat::Text,
            SourceRenderOptions {
                enabled: true,
                ..SourceRenderOptions::default()
            },
        )
        .unwrap();
        assert!(text.contains("Errors: 1"));
        assert!(text.contains("Toolchain:"));
        assert!(text.contains("DWARF:"));
        assert!(text.contains("Top source file growth:"));
        let markdown = build_ci_summary(
            &analysis,
            Some(&diff),
            CiFormat::Markdown,
            SourceRenderOptions {
                enabled: true,
                ..SourceRenderOptions::default()
            },
        )
        .unwrap();
        assert!(markdown.contains("# fwmap CI Summary"));
        assert!(markdown.contains("| Toolchain |"));
        assert!(markdown.contains("## Source Diff"));
        let json = build_ci_summary(
            &analysis,
            Some(&diff),
            CiFormat::Json,
            SourceRenderOptions {
                enabled: true,
                ..SourceRenderOptions::default()
            },
        )
        .unwrap();
        assert!(json.contains("\"error_count\""));
        assert!(json.contains("\"top_source_file_growth\""));
    }

    fn sample_analysis() -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "sample.elf".to_string(),
                arch: "ARM".to_string(),
                elf_class: "ELF32".to_string(),
                endian: "little-endian".to_string(),
            },
            toolchain: ToolchainInfo {
                requested: ToolchainSelection::Auto,
                detected: Some(ToolchainKind::Gnu),
                resolved: ToolchainKind::Gnu,
                linker_family: crate::model::LinkerFamily::Gnu,
                map_format: crate::model::MapFormat::Unknown,
                parser_warnings_count: 0,
            },
            debug_info: DebugInfoSummary::default(),
            debug_artifact: DebugArtifactInfo::default(),
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
                addr: 0x8000,
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
            compilation_units: Vec::new(),
            source_files: Vec::new(),
            line_attributions: Vec::new(),
            line_hotspots: Vec::new(),
            function_attributions: Vec::new(),
            unknown_source: UnknownSourceBucket::default(),
            warnings: Vec::new(),
        }
    }

    fn sample_analysis_with_sources() -> AnalysisResult {
        let mut analysis = sample_analysis();
        analysis.debug_info.dwarf_used = true;
        analysis.source_files = vec![crate::model::SourceFile {
            path: "src/main.cpp".to_string(),
            display_path: "src/main.cpp".to_string(),
            directory: "src".to_string(),
            size: 64,
            functions: 1,
            line_ranges: 2,
        }];
        analysis.function_attributions = vec![crate::model::FunctionAttribution {
            raw_name: "_ZN3app4mainEv".to_string(),
            demangled_name: Some("app::main()".to_string()),
            path: Some("src/main.cpp".to_string()),
            size: 64,
            ranges: vec![crate::model::SourceSpan {
                path: "src/main.cpp".to_string(),
                line_start: 10,
                line_end: 12,
                column: None,
            }],
        }];
        analysis.line_hotspots = vec![crate::model::LineRangeAttribution {
            path: "src/main.cpp".to_string(),
            line_start: 10,
            line_end: 12,
            section_name: Some(".text".to_string()),
            size: 64,
        }];
        analysis
    }
}
