use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

use crate::model::{AnalysisResult, WarningLevel};

#[derive(Debug, Clone)]
pub struct HistoryRecordInput {
    pub analysis: AnalysisResult,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildRecord {
    pub id: i64,
    pub created_at: i64,
    pub elf_path: String,
    pub arch: String,
    pub linker_family: String,
    pub map_format: String,
    pub rom_bytes: u64,
    pub ram_bytes: u64,
    pub warning_count: u64,
    pub error_count: u64,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildDetail {
    pub build: BuildRecord,
    pub debug_info: BuildDebugInfo,
    pub top_sections: Vec<(String, u64)>,
    pub regions: Vec<(String, u64, u64, f64)>,
    pub top_source_files: Vec<(String, u64, usize, usize)>,
    pub top_functions: Vec<(String, String, u64)>,
    pub warnings: Vec<(String, String, Option<String>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrendPoint {
    pub build_id: i64,
    pub created_at: i64,
    pub label: String,
    pub value: i64,
    pub format: TrendFormat,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildDebugInfo {
    pub dwarf_used: bool,
    pub unknown_source_ratio: f64,
    pub compilation_units: usize,
    pub source_file_count: usize,
    pub function_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendFormat {
    Bytes,
    Count,
    Percent,
}

pub fn record_build(db_path: &Path, input: HistoryRecordInput) -> Result<i64, String> {
    let mut conn = open_history_db(db_path)?;
    init_schema(&conn)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("failed to start history transaction: {err}"))?;

    let created_at = now_unix();
    let mut stored_metadata = input.metadata.clone();
    if input.analysis.debug_artifact.kind != crate::model::DebugArtifactKind::None {
        stored_metadata.insert("debug_artifact.kind".to_string(), input.analysis.debug_artifact.kind.to_string());
        stored_metadata.insert("debug_artifact.source".to_string(), input.analysis.debug_artifact.source.to_string());
        if let Some(path) = input.analysis.debug_artifact.path.as_deref() {
            stored_metadata.insert("debug_artifact.path".to_string(), path.to_string());
        }
        if let Some(build_id) = input.analysis.debug_artifact.build_id.as_deref() {
            stored_metadata.insert("debug_artifact.build_id".to_string(), build_id.to_string());
        }
    }
    let metadata_json =
        serde_json::to_string(&stored_metadata).map_err(|err| format!("failed to serialize history metadata: {err}"))?;
    let warning_count = input.analysis.warnings.len() as i64;
    let error_count = input
        .analysis
        .warnings
        .iter()
        .filter(|item| item.level == WarningLevel::Error)
        .count() as i64;

    tx.execute(
        "INSERT INTO builds (
            created_at, elf_path, arch, linker_family, map_format, rom_bytes, ram_bytes, warning_count, error_count, metadata_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            created_at,
            input.analysis.binary.path,
            input.analysis.binary.arch,
            input.analysis.toolchain.linker_family.to_string(),
            input.analysis.toolchain.map_format.to_string(),
            input.analysis.memory.rom_bytes as i64,
            input.analysis.memory.ram_bytes as i64,
            warning_count,
            error_count,
            metadata_json
        ],
    )
    .map_err(|err| format!("failed to insert build history: {err}"))?;
    let build_id = tx.last_insert_rowid();

    {
        // Keep source aggregates in separate tables so existing history databases can
        // migrate forward by simply creating the new tables on first access.
        let mut debug_stmt = tx
            .prepare(
                "INSERT INTO debug_metrics (
                    build_id, dwarf_used, unknown_source_ratio, compilation_units, source_file_count, function_count
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(|err| format!("failed to prepare debug insert: {err}"))?;
        debug_stmt
            .execute(params![
                build_id,
                input.analysis.debug_info.dwarf_used as i64,
                input.analysis.debug_info.unknown_source_ratio,
                input.analysis.debug_info.compilation_units as i64,
                input.analysis.source_files.len() as i64,
                input.analysis.function_attributions.len() as i64
            ])
            .map_err(|err| format!("failed to insert debug metric: {err}"))?;
    }

    {
        let mut section_stmt = tx
            .prepare("INSERT INTO section_metrics (build_id, section_name, size_bytes, category) VALUES (?1, ?2, ?3, ?4)")
            .map_err(|err| format!("failed to prepare section insert: {err}"))?;
        for section in &input.analysis.memory.section_totals {
            section_stmt
                .execute(params![build_id, section.section_name, section.size as i64, section.category.to_string()])
                .map_err(|err| format!("failed to insert section metric: {err}"))?;
        }
    }

    {
        let mut region_stmt = tx
            .prepare(
                "INSERT INTO region_metrics (build_id, region_name, used_bytes, free_bytes, usage_ratio)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(|err| format!("failed to prepare region insert: {err}"))?;
        for region in &input.analysis.memory.region_summaries {
            region_stmt
                .execute(params![
                    build_id,
                    region.region_name,
                    region.used as i64,
                    region.free as i64,
                    region.usage_ratio
                ])
                .map_err(|err| format!("failed to insert region metric: {err}"))?;
        }
    }

    {
        let mut source_stmt = tx
            .prepare(
                "INSERT INTO source_file_metrics (
                    build_id, path, display_path, directory, size_bytes, function_count, line_range_count
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .map_err(|err| format!("failed to prepare source file insert: {err}"))?;
        for source in &input.analysis.source_files {
            source_stmt
                .execute(params![
                    build_id,
                    source.path,
                    source.display_path,
                    source.directory,
                    source.size as i64,
                    source.functions as i64,
                    source.line_ranges as i64
                ])
                .map_err(|err| format!("failed to insert source file metric: {err}"))?;
        }
    }

    {
        let mut function_stmt = tx
            .prepare(
                "INSERT INTO function_metrics (
                    build_id, function_key, raw_name, demangled_name, path, size_bytes
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(|err| format!("failed to prepare function insert: {err}"))?;
        for function in &input.analysis.function_attributions {
            let function_key = match function.path.as_deref() {
                Some(path) => format!("{path}::{}", function.raw_name),
                None => function.raw_name.clone(),
            };
            function_stmt
                .execute(params![
                    build_id,
                    function_key,
                    function.raw_name,
                    function.demangled_name,
                    function.path,
                    function.size as i64
                ])
                .map_err(|err| format!("failed to insert function metric: {err}"))?;
        }
    }

    {
        let mut warning_stmt = tx
            .prepare(
                "INSERT INTO rule_results (build_id, code, level, related, message)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(|err| format!("failed to prepare rule insert: {err}"))?;
        for warning in &input.analysis.warnings {
            warning_stmt
                .execute(params![
                    build_id,
                    warning.code,
                    warning.level.to_string(),
                    warning.related,
                    warning.message
                ])
                .map_err(|err| format!("failed to insert rule result: {err}"))?;
        }
    }

    tx.commit()
        .map_err(|err| format!("failed to commit build history transaction: {err}"))?;
    Ok(build_id)
}

pub fn list_builds(db_path: &Path) -> Result<Vec<BuildRecord>, String> {
    let conn = open_history_db(db_path)?;
    init_schema(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, created_at, elf_path, arch, rom_bytes, ram_bytes, warning_count, error_count, metadata_json, linker_family, map_format
             FROM builds ORDER BY id DESC",
        )
        .map_err(|err| format!("failed to query build history: {err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(BuildRecord {
                id: row.get(0)?,
                created_at: row.get(1)?,
                elf_path: row.get(2)?,
                arch: row.get(3)?,
                linker_family: row.get(9)?,
                map_format: row.get(10)?,
                rom_bytes: row.get::<_, i64>(4)? as u64,
                ram_bytes: row.get::<_, i64>(5)? as u64,
                warning_count: row.get::<_, i64>(6)? as u64,
                error_count: row.get::<_, i64>(7)? as u64,
                metadata: parse_metadata(row.get::<_, String>(8)?),
            })
        })
        .map_err(|err| format!("failed to map build history rows: {err}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect build history rows: {err}"))
}

pub fn show_build(db_path: &Path, build_id: i64) -> Result<Option<BuildDetail>, String> {
    let conn = open_history_db(db_path)?;
    init_schema(&conn)?;
    let build = conn
        .query_row(
            "SELECT id, created_at, elf_path, arch, rom_bytes, ram_bytes, warning_count, error_count, metadata_json, linker_family, map_format
             FROM builds WHERE id = ?1",
            params![build_id],
            |row| {
                Ok(BuildRecord {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    elf_path: row.get(2)?,
                    arch: row.get(3)?,
                    linker_family: row.get(9)?,
                    map_format: row.get(10)?,
                    rom_bytes: row.get::<_, i64>(4)? as u64,
                    ram_bytes: row.get::<_, i64>(5)? as u64,
                    warning_count: row.get::<_, i64>(6)? as u64,
                    error_count: row.get::<_, i64>(7)? as u64,
                    metadata: parse_metadata(row.get::<_, String>(8)?),
                })
            },
        )
        .optional()
        .map_err(|err| format!("failed to query build detail: {err}"))?;

    let Some(build) = build else {
        return Ok(None);
    };

    let debug_info = conn
        .query_row(
            "SELECT dwarf_used, unknown_source_ratio, compilation_units, source_file_count, function_count
             FROM debug_metrics WHERE build_id = ?1",
            params![build_id],
            |row| {
                Ok(BuildDebugInfo {
                    dwarf_used: row.get::<_, i64>(0)? != 0,
                    unknown_source_ratio: row.get(1)?,
                    compilation_units: row.get::<_, i64>(2)? as usize,
                    source_file_count: row.get::<_, i64>(3)? as usize,
                    function_count: row.get::<_, i64>(4)? as usize,
                })
            },
        )
        .optional()
        .map_err(|err| format!("failed to query debug detail: {err}"))?
        .unwrap_or(BuildDebugInfo {
            dwarf_used: false,
            unknown_source_ratio: 0.0,
            compilation_units: 0,
            source_file_count: 0,
            function_count: 0,
        });

    let top_sections = query_pairs_i64(
        &conn,
        "SELECT section_name, size_bytes FROM section_metrics WHERE build_id = ?1 ORDER BY size_bytes DESC, section_name ASC LIMIT 10",
        build_id,
    )?;
    let regions = {
        let mut stmt = conn
            .prepare(
                "SELECT region_name, used_bytes, free_bytes, usage_ratio
                 FROM region_metrics WHERE build_id = ?1 ORDER BY used_bytes DESC, region_name ASC",
            )
            .map_err(|err| format!("failed to prepare region detail query: {err}"))?;
        let rows = stmt
            .query_map(params![build_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, i64>(2)? as u64,
                    row.get::<_, f64>(3)?,
                ))
            })
            .map_err(|err| format!("failed to query region detail: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to collect region detail: {err}"))?
    };
    let top_source_files = {
        let mut stmt = conn
            .prepare(
                "SELECT display_path, size_bytes, function_count, line_range_count
                 FROM source_file_metrics WHERE build_id = ?1 ORDER BY size_bytes DESC, display_path ASC LIMIT 10",
            )
            .map_err(|err| format!("failed to prepare source file detail query: {err}"))?;
        let rows = stmt
            .query_map(params![build_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, i64>(2)? as usize,
                    row.get::<_, i64>(3)? as usize,
                ))
            })
            .map_err(|err| format!("failed to query source file detail: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to collect source file detail: {err}"))?
    };
    let top_functions = {
        let mut stmt = conn
            .prepare(
                "SELECT COALESCE(demangled_name, raw_name), COALESCE(path, '-'), size_bytes
                 FROM function_metrics WHERE build_id = ?1 ORDER BY size_bytes DESC, raw_name ASC LIMIT 10",
            )
            .map_err(|err| format!("failed to prepare function detail query: {err}"))?;
        let rows = stmt
            .query_map(params![build_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? as u64))
            })
            .map_err(|err| format!("failed to query function detail: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to collect function detail: {err}"))?
    };
    let warnings = {
        let mut stmt = conn
            .prepare(
                "SELECT code, level, related FROM rule_results WHERE build_id = ?1 ORDER BY id ASC LIMIT 20",
            )
            .map_err(|err| format!("failed to prepare rule detail query: {err}"))?;
        let rows = stmt
            .query_map(params![build_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|err| format!("failed to query rule detail: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to collect rule detail: {err}"))?
    };

    Ok(Some(BuildDetail {
        build,
        debug_info,
        top_sections,
        regions,
        top_source_files,
        top_functions,
        warnings,
    }))
}

pub fn trend_metric(db_path: &Path, metric: &str, last: usize) -> Result<Vec<TrendPoint>, String> {
    let conn = open_history_db(db_path)?;
    init_schema(&conn)?;
    if metric.eq_ignore_ascii_case("rom") {
        return query_simple_trend(&conn, "rom_bytes", "rom", last, TrendFormat::Bytes);
    }
    if metric.eq_ignore_ascii_case("ram") {
        return query_simple_trend(&conn, "ram_bytes", "ram", last, TrendFormat::Bytes);
    }
    if metric.eq_ignore_ascii_case("warnings") {
        return query_simple_trend(&conn, "warning_count", "warnings", last, TrendFormat::Count);
    }
    if metric.eq_ignore_ascii_case("unknown_source") || metric.eq_ignore_ascii_case("unknown_source_ratio") {
        return query_unknown_source_trend(&conn, last);
    }
    if let Some(region) = metric.strip_prefix("region:") {
        return query_named_metric(
            &conn,
            "region_metrics",
            "region_name",
            "used_bytes",
            region,
            last,
            TrendFormat::Bytes,
            NamedMetricMode::ByName,
        );
    }
    if let Some(section) = metric.strip_prefix("section:") {
        return query_named_metric(
            &conn,
            "section_metrics",
            "section_name",
            "size_bytes",
            section,
            last,
            TrendFormat::Bytes,
            NamedMetricMode::ByName,
        );
    }
    if let Some(path) = metric.strip_prefix("source:") {
        return query_named_metric(
            &conn,
            "source_file_metrics",
            "path",
            "size_bytes",
            path,
            last,
            TrendFormat::Bytes,
            NamedMetricMode::ByName,
        );
    }
    if let Some(path) = metric.strip_prefix("function:") {
        return query_named_metric(
            &conn,
            "function_metrics",
            "function_key",
            "size_bytes",
            path,
            last,
            TrendFormat::Bytes,
            NamedMetricMode::ByName,
        );
    }
    if let Some(directory) = metric.strip_prefix("directory:") {
        return query_directory_trend(&conn, directory, last);
    }
    Err(format!(
        "unsupported trend metric '{metric}', expected rom|ram|warnings|unknown_source|region:<name>|section:<name>|source:<path>|function:<key>|directory:<path>"
    ))
}

fn query_simple_trend(
    conn: &Connection,
    column: &str,
    label: &str,
    last: usize,
    format: TrendFormat,
) -> Result<Vec<TrendPoint>, String> {
    let sql = format!(
        "SELECT id, created_at, {column} FROM builds ORDER BY id DESC LIMIT ?1"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|err| format!("failed to prepare trend query: {err}"))?;
    let rows = stmt
        .query_map(params![last as i64], |row| {
            Ok(TrendPoint {
                build_id: row.get(0)?,
                created_at: row.get(1)?,
                label: label.to_string(),
                value: row.get::<_, i64>(2)?,
                format,
            })
        })
        .map_err(|err| format!("failed to query trend metric: {err}"))?;
    let mut points = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect trend metric: {err}"))?;
    points.reverse();
    Ok(points)
}

fn query_named_metric(
    conn: &Connection,
    table: &str,
    name_column: &str,
    value_column: &str,
    name: &str,
    last: usize,
    format: TrendFormat,
    mode: NamedMetricMode,
) -> Result<Vec<TrendPoint>, String> {
    let sql = format!(
        "SELECT b.id, b.created_at, t.{value_column}
         FROM builds b
         JOIN {table} t ON t.build_id = b.id
         WHERE t.{name_column} = ?1
         ORDER BY b.id DESC LIMIT ?2"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|err| format!("failed to prepare named trend query: {err}"))?;
    let mut points = match mode {
        NamedMetricMode::ByName => stmt
            .query_map(params![name, last as i64], |row| {
                Ok(TrendPoint {
                    build_id: row.get(0)?,
                    created_at: row.get(1)?,
                    label: name.to_string(),
                    value: row.get::<_, i64>(2)?,
                    format,
                })
            })
            .map_err(|err| format!("failed to query named trend metric: {err}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to collect named trend metric: {err}"))?,
    };
    points.reverse();
    Ok(points)
}

fn query_directory_trend(conn: &Connection, directory: &str, last: usize) -> Result<Vec<TrendPoint>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.created_at, COALESCE(SUM(s.size_bytes), 0)
             FROM builds b
             LEFT JOIN source_file_metrics s ON s.build_id = b.id AND s.directory = ?1
             GROUP BY b.id, b.created_at
             ORDER BY b.id DESC LIMIT ?2",
        )
        .map_err(|err| format!("failed to prepare directory trend query: {err}"))?;
    let rows = stmt
        .query_map(params![directory, last as i64], |row| {
            Ok(TrendPoint {
                build_id: row.get(0)?,
                created_at: row.get(1)?,
                label: directory.to_string(),
                value: row.get::<_, i64>(2)?,
                format: TrendFormat::Bytes,
            })
        })
        .map_err(|err| format!("failed to query directory trend metric: {err}"))?;
    let mut points = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect directory trend metric: {err}"))?;
    points.reverse();
    Ok(points)
}

fn query_unknown_source_trend(conn: &Connection, last: usize) -> Result<Vec<TrendPoint>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.created_at, CAST(d.unknown_source_ratio * 1000.0 AS INTEGER)
             FROM builds b
             JOIN debug_metrics d ON d.build_id = b.id
             ORDER BY b.id DESC LIMIT ?1",
        )
        .map_err(|err| format!("failed to prepare unknown-source trend query: {err}"))?;
    let rows = stmt
        .query_map(params![last as i64], |row| {
            Ok(TrendPoint {
                build_id: row.get(0)?,
                created_at: row.get(1)?,
                label: "unknown_source".to_string(),
                value: row.get::<_, i64>(2)?,
                format: TrendFormat::Percent,
            })
        })
        .map_err(|err| format!("failed to query unknown-source trend metric: {err}"))?;
    let mut points = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect unknown-source trend metric: {err}"))?;
    points.reverse();
    Ok(points)
}

pub fn print_build_list(items: &[BuildRecord]) {
    if items.is_empty() {
        println!("No history records.");
        return;
    }
    for item in items {
        println!(
            "#{} {} ROM={} RAM={} warnings={} errors={} {} [{} / {}]",
            item.id,
            item.created_at,
            item.rom_bytes,
            item.ram_bytes,
            item.warning_count,
            item.error_count,
            item.elf_path,
            item.linker_family,
            item.map_format
        );
    }
}

pub fn print_build_detail(detail: &BuildDetail) {
    println!(
        "Build #{} at {}",
        detail.build.id, detail.build.created_at
    );
    println!(
        "ELF: {} | Arch: {} | Linker: {} | Map: {} | ROM: {} | RAM: {} | Warnings: {} | Errors: {}",
        detail.build.elf_path,
        detail.build.arch,
        detail.build.linker_family,
        detail.build.map_format,
        detail.build.rom_bytes,
        detail.build.ram_bytes,
        detail.build.warning_count,
        detail.build.error_count
    );
    println!(
        "DWARF: {} | Unknown ratio: {:.1}% | CUs: {} | Source files: {} | Functions: {}",
        if detail.debug_info.dwarf_used { "used" } else { "not used" },
        detail.debug_info.unknown_source_ratio * 100.0,
        detail.debug_info.compilation_units,
        detail.debug_info.source_file_count,
        detail.debug_info.function_count
    );
    if !detail.build.metadata.is_empty() {
        println!("Metadata:");
        for (key, value) in &detail.build.metadata {
            println!("  {}={}", key, value);
        }
    }
    if !detail.top_sections.is_empty() {
        println!("Top sections:");
        for (name, size) in &detail.top_sections {
            println!("  {} {}", name, size);
        }
    }
    if !detail.regions.is_empty() {
        println!("Regions:");
        for (name, used, free, usage_ratio) in &detail.regions {
            println!("  {} used={} free={} usage={:.1}%", name, used, free, usage_ratio * 100.0);
        }
    }
    if !detail.top_source_files.is_empty() {
        println!("Top source files:");
        for (path, size, functions, ranges) in &detail.top_source_files {
            println!("  {} {} functions={} line_ranges={}", path, size, functions, ranges);
        }
    }
    if !detail.top_functions.is_empty() {
        println!("Top functions:");
        for (name, path, size) in &detail.top_functions {
            println!("  {} {} {}", name, path, size);
        }
    }
    if !detail.warnings.is_empty() {
        println!("Warnings:");
        for (code, level, related) in &detail.warnings {
            println!("  {} [{}] {}", code, level, related.as_deref().unwrap_or("-"));
        }
    }
}

pub fn print_trend(points: &[TrendPoint]) {
    if points.is_empty() {
        println!("No trend points.");
        return;
    }
    for point in points {
        match point.format {
            TrendFormat::Bytes => println!("#{} {} {}={}", point.build_id, point.created_at, point.label, point.value),
            TrendFormat::Count => println!("#{} {} {}={}", point.build_id, point.created_at, point.label, point.value),
            TrendFormat::Percent => println!(
                "#{} {} {}={:.1}%",
                point.build_id,
                point.created_at,
                point.label,
                point.value as f64 / 10.0
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamedMetricMode {
    ByName,
}

fn query_pairs_i64(conn: &Connection, sql: &str, build_id: i64) -> Result<Vec<(String, u64)>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|err| format!("failed to prepare history pair query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64)))
        .map_err(|err| format!("failed to query history pairs: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect history pairs: {err}"))
}

fn open_history_db(path: &Path) -> Result<Connection, String> {
    Connection::open(path).map_err(|err| format!("failed to open history database '{}': {err}", path.display()))
}

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS builds (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at INTEGER NOT NULL,
            elf_path TEXT NOT NULL,
            arch TEXT NOT NULL,
            linker_family TEXT NOT NULL DEFAULT 'unknown',
            map_format TEXT NOT NULL DEFAULT 'unknown',
            rom_bytes INTEGER NOT NULL,
            ram_bytes INTEGER NOT NULL,
            warning_count INTEGER NOT NULL,
            error_count INTEGER NOT NULL,
            metadata_json TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS section_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            build_id INTEGER NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
            section_name TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            category TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS region_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            build_id INTEGER NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
            region_name TEXT NOT NULL,
            used_bytes INTEGER NOT NULL,
            free_bytes INTEGER NOT NULL,
            usage_ratio REAL NOT NULL
        );
        CREATE TABLE IF NOT EXISTS rule_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            build_id INTEGER NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
            code TEXT NOT NULL,
            level TEXT NOT NULL,
            related TEXT,
            message TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS debug_metrics (
            build_id INTEGER PRIMARY KEY REFERENCES builds(id) ON DELETE CASCADE,
            dwarf_used INTEGER NOT NULL,
            unknown_source_ratio REAL NOT NULL,
            compilation_units INTEGER NOT NULL,
            source_file_count INTEGER NOT NULL,
            function_count INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS source_file_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            build_id INTEGER NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
            path TEXT NOT NULL,
            display_path TEXT NOT NULL,
            directory TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            function_count INTEGER NOT NULL,
            line_range_count INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS function_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            build_id INTEGER NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
            function_key TEXT NOT NULL,
            raw_name TEXT NOT NULL,
            demangled_name TEXT,
            path TEXT,
            size_bytes INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS schema_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        INSERT INTO schema_meta(key, value) VALUES ('history_schema_version', '2')
        ON CONFLICT(key) DO UPDATE SET value = excluded.value;
        ",
    )
    .map_err(|err| format!("failed to initialize history database schema: {err}"))?;
    ensure_builds_column(conn, "linker_family", "TEXT NOT NULL DEFAULT 'unknown'")?;
    ensure_builds_column(conn, "map_format", "TEXT NOT NULL DEFAULT 'unknown'")?;
    Ok(())
}

fn ensure_builds_column(conn: &Connection, name: &str, definition: &str) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(builds)")
        .map_err(|err| format!("failed to inspect builds schema: {err}"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|err| format!("failed to query builds schema: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect builds schema: {err}"))?;
    if columns.iter().any(|column| column == name) {
        return Ok(());
    }
    conn.execute(
        &format!("ALTER TABLE builds ADD COLUMN {name} {definition}"),
        [],
    )
    .map_err(|err| format!("failed to migrate builds schema with column '{name}': {err}"))?;
    Ok(())
}

fn parse_metadata(raw: String) -> BTreeMap<String, String> {
    serde_json::from_str(&raw).unwrap_or_default()
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::{list_builds, record_build, show_build, trend_metric, HistoryRecordInput, TrendFormat};
    use crate::model::{
        AnalysisResult, BinaryInfo, DebugArtifactInfo, DebugInfoSummary, MemorySummary, SectionCategory, SectionTotal,
        SymbolInfo, ToolchainInfo, ToolchainKind, ToolchainSelection, UnknownSourceBucket, WarningItem, WarningLevel,
        WarningSource,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn records_lists_and_shows_builds() {
        let db = temp_db();
        let mut metadata = BTreeMap::new();
        metadata.insert("commit".to_string(), "abc123".to_string());
        let id = record_build(
            &db,
            HistoryRecordInput {
                analysis: sample_analysis(1024, 256, 1),
                metadata,
            },
        )
        .unwrap();
        let items = list_builds(&db).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, id);
        let detail = show_build(&db, id).unwrap().unwrap();
        assert_eq!(detail.build.metadata.get("commit").map(String::as_str), Some("abc123"));
        assert_eq!(detail.debug_info.source_file_count, 1);
        assert_eq!(detail.top_source_files.len(), 1);
        assert_eq!(detail.top_functions.len(), 1);
        let _ = fs::remove_file(db);
    }

    #[test]
    fn returns_trend_points() {
        let db = temp_db();
        record_build(
            &db,
            HistoryRecordInput {
                analysis: sample_analysis(100, 10, 0),
                metadata: BTreeMap::new(),
            },
        )
        .unwrap();
        record_build(
            &db,
            HistoryRecordInput {
                analysis: sample_analysis(120, 20, 2),
                metadata: BTreeMap::new(),
            },
        )
        .unwrap();
        let rom = trend_metric(&db, "rom", 10).unwrap();
        assert_eq!(rom.len(), 2);
        assert_eq!(rom[1].value, 120);
        let warnings = trend_metric(&db, "warnings", 10).unwrap();
        assert_eq!(warnings[1].value, 2);
        let source = trend_metric(&db, "source:src/main.cpp", 10).unwrap();
        assert_eq!(source[1].value, 120);
        let unknown = trend_metric(&db, "unknown_source", 10).unwrap();
        assert_eq!(unknown[1].format, TrendFormat::Percent);
        let _ = fs::remove_file(db);
    }

    fn temp_db() -> std::path::PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("fwmap-history-{nanos}.db"))
    }

    fn sample_analysis(rom: u64, ram: u64, warnings: usize) -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "sample.elf".to_string(),
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
            debug_info: DebugInfoSummary {
                dwarf_mode: crate::model::DwarfMode::Auto,
                source_lines: crate::model::SourceLinesMode::All,
                dwarf_used: true,
                cache_hit: false,
                split_dwarf_detected: false,
                split_dwarf_kind: None,
                unknown_source_ratio: if rom + ram == 0 { 0.0 } else { ram as f64 / (rom + ram) as f64 },
                compilation_units: 1,
                line_zero_ranges: 0,
                generated_ranges: 0,
            },
            debug_artifact: DebugArtifactInfo::default(),
            sections: Vec::new(),
            symbols: vec![SymbolInfo {
                name: "main".to_string(),
                demangled_name: None,
                section_name: None,
                object_path: None,
                addr: 0,
                size: 32,
            }],
            object_contributions: Vec::new(),
            archive_contributions: Vec::new(),
            archive_pulls: Vec::new(),
            relocation_references: Vec::new(),
            cross_references: Vec::new(),
            linker_script: None,
            memory: MemorySummary {
                rom_bytes: rom,
                ram_bytes: ram,
                section_totals: vec![
                    SectionTotal {
                        section_name: ".text".to_string(),
                        size: rom,
                        category: SectionCategory::Rom,
                    },
                    SectionTotal {
                        section_name: ".data".to_string(),
                        size: ram,
                        category: SectionCategory::Ram,
                    },
                ],
                memory_regions: Vec::new(),
                region_summaries: Vec::new(),
            },
            compilation_units: Vec::new(),
            source_files: vec![crate::model::SourceFile {
                path: "src/main.cpp".to_string(),
                display_path: "src/main.cpp".to_string(),
                directory: "src".to_string(),
                size: rom,
                functions: 1,
                line_ranges: 1,
            }],
            line_attributions: Vec::new(),
            line_hotspots: Vec::new(),
            function_attributions: vec![crate::model::FunctionAttribution {
                raw_name: "_ZN4mainEv".to_string(),
                demangled_name: Some("main()".to_string()),
                path: Some("src/main.cpp".to_string()),
                size: rom,
                ranges: vec![crate::model::SourceSpan {
                    path: "src/main.cpp".to_string(),
                    line_start: 10,
                    line_end: 12,
                    column: None,
                }],
            }],
            unknown_source: UnknownSourceBucket {
                size: ram,
                ranges: Vec::new(),
            },
            warnings: (0..warnings)
                .map(|index| WarningItem {
                    level: if index == warnings.saturating_sub(1) && warnings > 1 {
                        WarningLevel::Error
                    } else {
                        WarningLevel::Warn
                    },
                    code: format!("W{index}"),
                    message: "warning".to_string(),
                    source: WarningSource::Analyze,
                    related: None,
                })
                .collect(),
        }
    }
}
