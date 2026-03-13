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
    pub rom_bytes: u64,
    pub ram_bytes: u64,
    pub warning_count: u64,
    pub error_count: u64,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildDetail {
    pub build: BuildRecord,
    pub top_sections: Vec<(String, u64)>,
    pub regions: Vec<(String, u64, u64, f64)>,
    pub warnings: Vec<(String, String, Option<String>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrendPoint {
    pub build_id: i64,
    pub created_at: i64,
    pub label: String,
    pub value: i64,
}

pub fn record_build(db_path: &Path, input: HistoryRecordInput) -> Result<i64, String> {
    let conn = open_history_db(db_path)?;
    init_schema(&conn)?;

    let created_at = now_unix();
    let metadata_json =
        serde_json::to_string(&input.metadata).map_err(|err| format!("failed to serialize history metadata: {err}"))?;
    let warning_count = input.analysis.warnings.len() as i64;
    let error_count = input
        .analysis
        .warnings
        .iter()
        .filter(|item| item.level == WarningLevel::Error)
        .count() as i64;

    conn.execute(
        "INSERT INTO builds (created_at, elf_path, arch, rom_bytes, ram_bytes, warning_count, error_count, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            created_at,
            input.analysis.binary.path,
            input.analysis.binary.arch,
            input.analysis.memory.rom_bytes as i64,
            input.analysis.memory.ram_bytes as i64,
            warning_count,
            error_count,
            metadata_json
        ],
    )
    .map_err(|err| format!("failed to insert build history: {err}"))?;
    let build_id = conn.last_insert_rowid();

    {
        let mut section_stmt = conn
            .prepare("INSERT INTO section_metrics (build_id, section_name, size_bytes, category) VALUES (?1, ?2, ?3, ?4)")
            .map_err(|err| format!("failed to prepare section insert: {err}"))?;
        for section in &input.analysis.memory.section_totals {
            section_stmt
                .execute(params![build_id, section.section_name, section.size as i64, section.category.to_string()])
                .map_err(|err| format!("failed to insert section metric: {err}"))?;
        }
    }

    {
        let mut region_stmt = conn
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
        let mut warning_stmt = conn
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

    Ok(build_id)
}

pub fn list_builds(db_path: &Path) -> Result<Vec<BuildRecord>, String> {
    let conn = open_history_db(db_path)?;
    init_schema(&conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, created_at, elf_path, arch, rom_bytes, ram_bytes, warning_count, error_count, metadata_json
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
            "SELECT id, created_at, elf_path, arch, rom_bytes, ram_bytes, warning_count, error_count, metadata_json
             FROM builds WHERE id = ?1",
            params![build_id],
            |row| {
                Ok(BuildRecord {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    elf_path: row.get(2)?,
                    arch: row.get(3)?,
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
        top_sections,
        regions,
        warnings,
    }))
}

pub fn trend_metric(db_path: &Path, metric: &str, last: usize) -> Result<Vec<TrendPoint>, String> {
    let conn = open_history_db(db_path)?;
    init_schema(&conn)?;
    if metric.eq_ignore_ascii_case("rom") {
        return query_simple_trend(&conn, "rom_bytes", "rom", last);
    }
    if metric.eq_ignore_ascii_case("ram") {
        return query_simple_trend(&conn, "ram_bytes", "ram", last);
    }
    if metric.eq_ignore_ascii_case("warnings") {
        return query_simple_trend(&conn, "warning_count", "warnings", last);
    }
    if let Some(region) = metric.strip_prefix("region:") {
        return query_named_metric(&conn, "region_metrics", "region_name", "used_bytes", region, last);
    }
    if let Some(section) = metric.strip_prefix("section:") {
        return query_named_metric(&conn, "section_metrics", "section_name", "size_bytes", section, last);
    }
    Err(format!(
        "unsupported trend metric '{metric}', expected rom|ram|warnings|region:<name>|section:<name>"
    ))
}

fn query_simple_trend(conn: &Connection, column: &str, label: &str, last: usize) -> Result<Vec<TrendPoint>, String> {
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
    let rows = stmt
        .query_map(params![name, last as i64], |row| {
            Ok(TrendPoint {
                build_id: row.get(0)?,
                created_at: row.get(1)?,
                label: name.to_string(),
                value: row.get::<_, i64>(2)?,
            })
        })
        .map_err(|err| format!("failed to query named trend metric: {err}"))?;
    let mut points = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect named trend metric: {err}"))?;
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
            "#{} {} ROM={} RAM={} warnings={} errors={} {}",
            item.id, item.created_at, item.rom_bytes, item.ram_bytes, item.warning_count, item.error_count, item.elf_path
        );
    }
}

pub fn print_build_detail(detail: &BuildDetail) {
    println!(
        "Build #{} at {}",
        detail.build.id, detail.build.created_at
    );
    println!(
        "ELF: {} | Arch: {} | ROM: {} | RAM: {} | Warnings: {} | Errors: {}",
        detail.build.elf_path,
        detail.build.arch,
        detail.build.rom_bytes,
        detail.build.ram_bytes,
        detail.build.warning_count,
        detail.build.error_count
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
        println!("#{} {} {}={}", point.build_id, point.created_at, point.label, point.value);
    }
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
        ",
    )
    .map_err(|err| format!("failed to initialize history database schema: {err}"))
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
    use super::{list_builds, record_build, show_build, trend_metric, HistoryRecordInput};
    use crate::model::{
        AnalysisResult, BinaryInfo, DebugInfoSummary, MemorySummary, SectionCategory, SectionTotal, SymbolInfo,
        ToolchainInfo, ToolchainKind, ToolchainSelection, UnknownSourceBucket, WarningItem, WarningLevel,
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
            },
            debug_info: DebugInfoSummary::default(),
            sections: Vec::new(),
            symbols: vec![SymbolInfo {
                name: "main".to_string(),
                demangled_name: None,
                section_name: None,
                object_path: None,
                size: 32,
            }],
            object_contributions: Vec::new(),
            archive_contributions: Vec::new(),
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
            source_files: Vec::new(),
            line_attributions: Vec::new(),
            function_attributions: Vec::new(),
            unknown_source: UnknownSourceBucket::default(),
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
