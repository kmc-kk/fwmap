use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::linkage::explain_object;
use crate::git::{changed_files, list_commits, list_range_commits, merge_base, resolve_repo_root, resolve_revision, CommitOrder, GitCommit};
use crate::model::{AnalysisResult, GitMetadata, RustContext, WarningLevel};

#[derive(Debug, Clone)]
pub struct HistoryRecordInput {
    pub analysis: AnalysisResult,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
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
    pub git: Option<GitMetadata>,
    pub rust_context: Option<RustContext>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildDetail {
    pub build: BuildRecord,
    pub debug_info: BuildDebugInfo,
    pub top_sections: Vec<(String, u64)>,
    pub regions: Vec<(String, u64, u64, f64)>,
    pub top_source_files: Vec<(String, u64, usize, usize)>,
    pub top_functions: Vec<(String, String, u64)>,
    pub rust_packages: Vec<(String, u64, usize)>,
    pub rust_targets: Vec<(String, u64, usize)>,
    pub rust_crates: Vec<(String, u64, usize)>,
    pub rust_dependencies: Vec<(String, u64, usize)>,
    pub rust_source_files: Vec<(String, u64, usize)>,
    pub rust_families: Vec<(String, u64, usize)>,
    pub why_linked: Vec<WhyLinkedRecord>,
    pub warnings: Vec<(String, String, Option<String>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhyLinkedRecord {
    pub target: String,
    pub kind: String,
    pub confidence: String,
    pub summary: String,
    pub current_size: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrendPoint {
    pub build_id: i64,
    pub created_at: i64,
    pub label: String,
    pub value: i64,
    pub format: TrendFormat,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeEntry {
    pub name: String,
    pub delta: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct TimelineTopIncreases {
    pub sections: Vec<ChangeEntry>,
    pub objects: Vec<ChangeEntry>,
    pub source_files: Vec<ChangeEntry>,
    pub symbols: Vec<ChangeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitTimelineRow {
    pub repo_id: String,
    pub commit: String,
    pub short_commit: String,
    pub commit_time: String,
    pub author_name: String,
    pub subject: String,
    pub branch_names: Vec<String>,
    pub tag_names: Vec<String>,
    pub describe: Option<String>,
    pub build_profile: Option<String>,
    pub toolchain_id: Option<String>,
    pub target_id: Option<String>,
    pub configuration_fingerprint: Option<String>,
    pub rom_total: u64,
    pub ram_total: u64,
    pub rom_delta_vs_previous: Option<i64>,
    pub ram_delta_vs_previous: Option<i64>,
    pub rule_violations_count: u64,
    pub top_increases: TimelineTopIncreases,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommitTimelineReport {
    pub repo_id: String,
    pub order: String,
    pub filters: TimelineFilters,
    pub rows: Vec<CommitTimelineRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct TimelineFilters {
    pub branch: Option<String>,
    pub profile: Option<String>,
    pub toolchain: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangedFilesSummary {
    pub git_changed_files: Vec<String>,
    pub changed_source_files_in_analysis: Vec<String>,
    pub intersection_files: Vec<String>,
    pub git_only_files_count: usize,
    pub analysis_only_files_count: usize,
    pub intersection_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorstCommitSummary {
    pub commit: String,
    pub delta: i64,
    pub subject: String,
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FirstRuleViolationSummary {
    pub commit: String,
    pub rule_ids: Vec<String>,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RangeDiffReport {
    pub repo_id: String,
    pub input_range_spec: String,
    pub comparison_mode: String,
    pub resolved_base: String,
    pub resolved_head: String,
    pub resolved_merge_base: Option<String>,
    pub order: String,
    pub total_commits_in_git_range: usize,
    pub analyzed_commits_count: usize,
    pub missing_analysis_commits_count: usize,
    pub first_analyzed_commit: Option<String>,
    pub last_analyzed_commit: Option<String>,
    pub cumulative_rom_delta: i64,
    pub cumulative_ram_delta: i64,
    pub worst_commit_by_rom: Option<WorstCommitSummary>,
    pub worst_commit_by_ram: Option<WorstCommitSummary>,
    pub first_rule_violation: Option<FirstRuleViolationSummary>,
    pub top_changed_sections: Vec<ChangeEntry>,
    pub top_changed_objects: Vec<ChangeEntry>,
    pub top_changed_source_files: Vec<ChangeEntry>,
    pub top_changed_symbols: Vec<ChangeEntry>,
    pub top_changed_rust_dependencies: Vec<ChangeEntry>,
    pub top_changed_rust_families: Vec<ChangeEntry>,
    pub changed_files_summary: Option<ChangedFilesSummary>,
    pub timeline_rows: Vec<CommitTimelineRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegressionMode {
    FirstCrossing,
    FirstJump,
    FirstPresence,
    FirstViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RegressionConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RegressionDetector {
    Metric,
    Rule,
    Entity,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegressionQuery {
    pub detector_type: RegressionDetector,
    pub key: String,
    pub mode: RegressionMode,
    pub range_spec: String,
    pub resolved_base: String,
    pub resolved_head: String,
    pub order: String,
    pub threshold: Option<i64>,
    pub threshold_percent: Option<f64>,
    pub jump_threshold: Option<i64>,
    pub include_evidence: bool,
    pub include_changed_files: bool,
    pub bisect_like: bool,
}

#[derive(Debug, Clone)]
struct RegressionPoint {
    commit_index: usize,
    commit: GitCommit,
    build: BuildRecord,
}

#[derive(Debug, Clone, Default)]
struct DetectionResult {
    values: Vec<Option<i64>>,
    last_good_index: Option<usize>,
    first_bad_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegressionOriginPoint {
    pub commit: String,
    pub short_commit: String,
    pub subject: String,
    pub value: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegressionSummary {
    pub searched_commit_count: usize,
    pub analyzed_commit_count: usize,
    pub missing_analysis_count: usize,
    pub confidence: RegressionConfidence,
    pub mixed_configuration: bool,
    pub reasoning: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegressionOrigin {
    pub last_good: Option<RegressionOriginPoint>,
    pub first_observed_bad: Option<RegressionOriginPoint>,
    pub first_bad_candidate: Option<RegressionOriginPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegressionWindowRow {
    pub commit: String,
    pub short_commit: String,
    pub subject: String,
    pub status: String,
    pub value: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct RegressionEvidence {
    pub transition_window: Vec<RegressionWindowRow>,
    pub top_growth: TimelineTopIncreases,
    pub changed_files: Option<ChangedFilesSummary>,
    pub related_rule_hits: Vec<String>,
    pub narrowed_commits: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegressionReport {
    pub repo_id: String,
    pub query: RegressionQuery,
    pub summary: RegressionSummary,
    pub origin: RegressionOrigin,
    pub evidence: Option<RegressionEvidence>,
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

    if let Some(git) = input.analysis.git.as_ref() {
        let tag_names_json =
            serde_json::to_string(&git.tag_names).map_err(|err| format!("failed to serialize git tags: {err}"))?;
        tx.execute(
            "INSERT INTO git_metadata (
                build_id, repo_root, commit_hash, short_commit_hash, branch_name, detached_head, tag_names_json,
                commit_subject, author_name, author_email, commit_timestamp, describe, is_dirty
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                build_id,
                git.repo_root,
                git.commit_hash,
                git.short_commit_hash,
                git.branch_name,
                git.detached_head as i64,
                tag_names_json,
                git.commit_subject,
                git.author_name,
                git.author_email,
                git.commit_timestamp,
                git.describe,
                git.is_dirty as i64
            ],
        )
        .map_err(|err| format!("failed to insert git metadata: {err}"))?;
    }

    if let Some(rust) = input.analysis.rust_context.as_ref() {
        let target_kind_json =
            serde_json::to_string(&rust.target_kind).map_err(|err| format!("failed to serialize Rust target_kind: {err}"))?;
        let crate_types_json =
            serde_json::to_string(&rust.crate_types).map_err(|err| format!("failed to serialize Rust crate_types: {err}"))?;
        let workspace_members_json = serde_json::to_string(&rust.workspace_members)
            .map_err(|err| format!("failed to serialize Rust workspace_members: {err}"))?;
        tx.execute(
            "INSERT INTO rust_metadata (
                build_id, workspace_root, manifest_path, package_name, package_id, target_name, target_kind_json,
                crate_types_json, edition, target_triple, profile, artifact_path, metadata_source, workspace_members_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                build_id,
                rust.workspace_root,
                rust.manifest_path,
                rust.package_name,
                rust.package_id,
                rust.target_name,
                target_kind_json,
                crate_types_json,
                rust.edition,
                rust.target_triple,
                rust.profile,
                rust.artifact_path,
                rust.metadata_source,
                workspace_members_json
            ],
        )
        .map_err(|err| format!("failed to insert Rust metadata: {err}"))?;
    }

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
        let mut object_stmt = tx
            .prepare("INSERT INTO object_metrics (build_id, object_path, size_bytes) VALUES (?1, ?2, ?3)")
            .map_err(|err| format!("failed to prepare object insert: {err}"))?;
        let mut object_totals = BTreeMap::<String, u64>::new();
        for object in &input.analysis.object_contributions {
            *object_totals.entry(object.object_path.clone()).or_default() += object.size;
        }
        for (object_path, size) in object_totals {
            object_stmt
                .execute(params![build_id, object_path, size as i64])
                .map_err(|err| format!("failed to insert object metric: {err}"))?;
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
        let mut symbol_stmt = tx
            .prepare(
                "INSERT INTO symbol_metrics (build_id, name, demangled_name, size_bytes) VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(|err| format!("failed to prepare symbol insert: {err}"))?;
        for symbol in &input.analysis.symbols {
            symbol_stmt
                .execute(params![build_id, symbol.name, symbol.demangled_name, symbol.size as i64])
                .map_err(|err| format!("failed to insert symbol metric: {err}"))?;
        }
    }

    {
        let mut why_stmt = tx
            .prepare(
                "INSERT INTO why_linked_metrics (
                    build_id, target, kind, confidence, summary, current_size
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .map_err(|err| format!("failed to prepare why-linked insert: {err}"))?;
        for record in collect_why_linked_records(&input.analysis, 20) {
            why_stmt
                .execute(params![
                    build_id,
                    record.target,
                    record.kind,
                    record.confidence,
                    record.summary,
                    record.current_size as i64
                ])
                .map_err(|err| format!("failed to insert why-linked metric: {err}"))?;
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

    if let Some(rust_view) = input.analysis.rust_view.as_ref() {
        let mut rust_stmt = tx
            .prepare(
                "INSERT INTO rust_aggregate_metrics (build_id, scope, name, size_bytes, symbol_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(|err| format!("failed to prepare Rust aggregate insert: {err}"))?;
        for item in &rust_view.packages {
            rust_stmt
                .execute(params![build_id, "package", item.name, item.size as i64, item.symbol_count as i64])
                .map_err(|err| format!("failed to insert Rust package aggregate: {err}"))?;
        }
        for item in &rust_view.targets {
            rust_stmt
                .execute(params![build_id, "target", item.name, item.size as i64, item.symbol_count as i64])
                .map_err(|err| format!("failed to insert Rust target aggregate: {err}"))?;
        }
        for item in &rust_view.crates {
            rust_stmt
                .execute(params![build_id, "crate", item.name, item.size as i64, item.symbol_count as i64])
                .map_err(|err| format!("failed to insert Rust crate aggregate: {err}"))?;
        }
        for item in &rust_view.source_files {
            rust_stmt
                .execute(params![build_id, "source", item.name, item.size as i64, item.symbol_count as i64])
                .map_err(|err| format!("failed to insert Rust source aggregate: {err}"))?;
        }
        for item in &rust_view.dependency_crates {
            rust_stmt
                .execute(params![build_id, "dependency", item.name, item.size as i64, item.symbol_count as i64])
                .map_err(|err| format!("failed to insert Rust dependency aggregate: {err}"))?;
        }
        for item in &rust_view.grouped_families {
            rust_stmt
                .execute(params![
                    build_id,
                    format!("family:{:?}", item.kind).to_lowercase(),
                    item.display_name,
                    item.size as i64,
                    item.symbol_count as i64
                ])
                .map_err(|err| format!("failed to insert Rust family aggregate: {err}"))?;
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
            "SELECT b.id, b.created_at, b.elf_path, b.arch, b.rom_bytes, b.ram_bytes, b.warning_count, b.error_count,
                    b.metadata_json, b.linker_family, b.map_format,
                    g.repo_root, g.commit_hash, g.short_commit_hash, g.branch_name, g.detached_head, g.tag_names_json,
                    g.commit_subject, g.author_name, g.author_email, g.commit_timestamp, g.describe, g.is_dirty,
                    r.workspace_root, r.manifest_path, r.package_name, r.package_id, r.target_name, r.target_kind_json,
                    r.crate_types_json, r.edition, r.target_triple, r.profile, r.artifact_path, r.metadata_source,
                    r.workspace_members_json
             FROM builds b
             LEFT JOIN git_metadata g ON g.build_id = b.id
             LEFT JOIN rust_metadata r ON r.build_id = b.id
             ORDER BY b.id DESC",
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
                git: parse_git_metadata(row)?,
                rust_context: parse_rust_metadata(row)?,
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
            "SELECT b.id, b.created_at, b.elf_path, b.arch, b.rom_bytes, b.ram_bytes, b.warning_count, b.error_count,
                    b.metadata_json, b.linker_family, b.map_format,
                    g.repo_root, g.commit_hash, g.short_commit_hash, g.branch_name, g.detached_head, g.tag_names_json,
                    g.commit_subject, g.author_name, g.author_email, g.commit_timestamp, g.describe, g.is_dirty,
                    r.workspace_root, r.manifest_path, r.package_name, r.package_id, r.target_name, r.target_kind_json,
                    r.crate_types_json, r.edition, r.target_triple, r.profile, r.artifact_path, r.metadata_source,
                    r.workspace_members_json
             FROM builds b
             LEFT JOIN git_metadata g ON g.build_id = b.id
             LEFT JOIN rust_metadata r ON r.build_id = b.id
             WHERE b.id = ?1",
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
                    git: parse_git_metadata(row)?,
                    rust_context: parse_rust_metadata(row)?,
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
    let rust_packages = query_triple_i64(
        &conn,
        "SELECT name, size_bytes, symbol_count FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope = 'package' ORDER BY size_bytes DESC, name ASC LIMIT 10",
        build_id,
    )?;
    let rust_targets = query_triple_i64(
        &conn,
        "SELECT name, size_bytes, symbol_count FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope = 'target' ORDER BY size_bytes DESC, name ASC LIMIT 10",
        build_id,
    )?;
    let rust_crates = query_triple_i64(
        &conn,
        "SELECT name, size_bytes, symbol_count FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope = 'crate' ORDER BY size_bytes DESC, name ASC LIMIT 10",
        build_id,
    )?;
    let rust_dependencies = query_triple_i64(
        &conn,
        "SELECT name, size_bytes, symbol_count FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope = 'dependency' ORDER BY size_bytes DESC, name ASC LIMIT 10",
        build_id,
    )?;
    let rust_source_files = query_triple_i64(
        &conn,
        "SELECT name, size_bytes, symbol_count FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope = 'source' ORDER BY size_bytes DESC, name ASC LIMIT 10",
        build_id,
    )?;
    let rust_families = query_triple_i64(
        &conn,
        "SELECT name, size_bytes, symbol_count FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope LIKE 'family:%' ORDER BY size_bytes DESC, name ASC LIMIT 10",
        build_id,
    )?;
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
    let why_linked = {
        let mut stmt = conn
            .prepare(
                "SELECT target, kind, confidence, summary, current_size
                 FROM why_linked_metrics WHERE build_id = ?1
                 ORDER BY current_size DESC, target ASC LIMIT 20",
            )
            .map_err(|err| format!("failed to prepare why-linked detail query: {err}"))?;
        let rows = stmt
            .query_map(params![build_id], |row| {
                Ok(WhyLinkedRecord {
                    target: row.get(0)?,
                    kind: row.get(1)?,
                    confidence: row.get(2)?,
                    summary: row.get(3)?,
                    current_size: row.get::<_, i64>(4)? as u64,
                })
            })
            .map_err(|err| format!("failed to query why-linked detail: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to collect why-linked detail: {err}"))?
    };

    Ok(Some(BuildDetail {
        build,
        debug_info,
        top_sections,
        regions,
        top_source_files,
        top_functions,
        rust_packages,
        rust_targets,
        rust_crates,
        rust_dependencies,
        rust_source_files,
        rust_families,
        why_linked,
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
    if let Some(path) = metric.strip_prefix("object:") {
        return query_why_linked_trend(&conn, path, "object", last);
    }
    if let Some(path) = metric.strip_prefix("archive-member:") {
        return query_why_linked_trend(&conn, path, "object", last);
    }
    if let Some(directory) = metric.strip_prefix("directory:") {
        return query_directory_trend(&conn, directory, last);
    }
    if let Some(name) = metric.strip_prefix("rust-package:") {
        return query_named_metric(
            &conn,
            "rust_aggregate_metrics",
            "name",
            "size_bytes",
            name,
            last,
            TrendFormat::Bytes,
            NamedMetricMode::ByNameAndScope("package"),
        );
    }
    if let Some(name) = metric.strip_prefix("rust-target:") {
        return query_named_metric(
            &conn,
            "rust_aggregate_metrics",
            "name",
            "size_bytes",
            name,
            last,
            TrendFormat::Bytes,
            NamedMetricMode::ByNameAndScope("target"),
        );
    }
    if let Some(name) = metric.strip_prefix("rust-crate:") {
        return query_named_metric(
            &conn,
            "rust_aggregate_metrics",
            "name",
            "size_bytes",
            name,
            last,
            TrendFormat::Bytes,
            NamedMetricMode::ByNameAndScope("crate"),
        );
    }
    if let Some(name) = metric.strip_prefix("rust-dependency:") {
        return query_named_metric(
            &conn,
            "rust_aggregate_metrics",
            "name",
            "size_bytes",
            name,
            last,
            TrendFormat::Bytes,
            NamedMetricMode::ByNameAndScope("dependency"),
        );
    }
    if let Some(name) = metric.strip_prefix("rust-source:") {
        return query_named_metric(
            &conn,
            "rust_aggregate_metrics",
            "name",
            "size_bytes",
            name,
            last,
            TrendFormat::Bytes,
            NamedMetricMode::ByNameAndScope("source"),
        );
    }
    if let Some(name) = metric.strip_prefix("rust-family:") {
        return query_named_metric_like_scope(&conn, name, "family:%", last);
    }
    Err(format!(
        "unsupported trend metric '{metric}', expected rom|ram|warnings|unknown_source|region:<name>|section:<name>|source:<path>|function:<key>|object:<path>|archive-member:<archive(member)>|directory:<path>|rust-package:<name>|rust-target:<name>|rust-crate:<name>|rust-dependency:<name>|rust-source:<path>|rust-family:<name>"
    ))
}

pub fn commit_timeline(
    db_path: &Path,
    repo: Option<&Path>,
    branch: Option<&str>,
    limit: usize,
    profile: Option<&str>,
    toolchain: Option<&str>,
    target: Option<&str>,
    order: CommitOrder,
) -> Result<CommitTimelineReport, String> {
    let repo_root = resolve_repo_root(repo).ok_or_else(|| "git repository was not found".to_string())?;
    let revision = branch.unwrap_or("HEAD");
    let commits = list_commits(repo, revision, limit, order)?;
    let all_builds = list_builds(db_path)?;
    let build_map = latest_build_by_commit(&all_builds, &repo_root);
    let mut timeline_items = Vec::<(GitCommit, BuildRecord)>::new();
    for commit in commits {
        let Some(build) = build_map.get(&commit.commit).cloned() else {
            continue;
        };
        if !matches_filters(&build, profile, toolchain, target, branch) {
            continue;
        }
        timeline_items.push((commit, build));
    }
    let mut rows = Vec::with_capacity(timeline_items.len());
    let mut next_older_by_variant = HashMap::<String, BuildRecord>::new();
    for (commit, build) in timeline_items.into_iter().rev() {
        let variant = variant_key(&build);
        let diff = next_older_by_variant
            .get(&variant)
            .map(|older| build_metric_diff(db_path, build.id, older.id))
            .transpose()?;
        rows.push(build_timeline_row(&repo_root, &commit, &build, diff.as_ref()));
        next_older_by_variant.insert(variant, build);
    }
    rows.reverse();
    Ok(CommitTimelineReport {
        repo_id: repo_root,
        order: commit_order_name(order).to_string(),
        filters: TimelineFilters {
            branch: branch.map(ToOwned::to_owned),
            profile: profile.map(ToOwned::to_owned),
            toolchain: toolchain.map(ToOwned::to_owned),
            target: target.map(ToOwned::to_owned),
        },
        rows,
    })
}

pub fn range_diff(
    db_path: &Path,
    repo: Option<&Path>,
    spec: &str,
    order: CommitOrder,
    include_changed_files: bool,
    profile: Option<&str>,
    toolchain: Option<&str>,
    target: Option<&str>,
) -> Result<RangeDiffReport, String> {
    let repo_root = resolve_repo_root(repo).ok_or_else(|| "git repository was not found".to_string())?;
    let resolved = resolve_range_spec(repo, spec)?;
    let commits = list_range_commits(repo, &resolved.git_range, order)?;
    let all_builds = list_builds(db_path)?;
    let build_map = latest_build_by_commit(&all_builds, &repo_root);
    let mut timeline_items = Vec::<(GitCommit, BuildRecord)>::new();
    let mut analyzed_builds = Vec::<BuildRecord>::new();
    for commit in commits.iter() {
        let Some(build) = build_map.get(&commit.commit).cloned() else {
            continue;
        };
        if !matches_filters(&build, profile, toolchain, target, None) {
            continue;
        }
        timeline_items.push((commit.clone(), build.clone()));
        analyzed_builds.push(build);
    }
    let mut rows = Vec::with_capacity(timeline_items.len());
    let mut next_older_by_variant = HashMap::<String, BuildRecord>::new();
    for (commit, build) in timeline_items.into_iter().rev() {
        let variant = variant_key(&build);
        let diff = next_older_by_variant
            .get(&variant)
            .map(|older| build_metric_diff(db_path, build.id, older.id))
            .transpose()?;
        rows.push(build_timeline_row(&repo_root, &commit, &build, diff.as_ref()));
        next_older_by_variant.insert(variant, build);
    }
    rows.reverse();
    let mut cumulative_rom_delta = 0i64;
    let mut cumulative_ram_delta = 0i64;
    let mut worst_commit_by_rom = None;
    let mut worst_commit_by_ram = None;
    for row in &rows {
        if let Some(delta) = row.rom_delta_vs_previous {
            cumulative_rom_delta += delta;
            if worst_commit_by_rom.as_ref().map(|item: &WorstCommitSummary| delta > item.delta).unwrap_or(true) {
                worst_commit_by_rom = Some(WorstCommitSummary {
                    commit: row.short_commit.clone(),
                    delta,
                    subject: row.subject.clone(),
                    date: row.commit_time.clone(),
                });
            }
        }
        if let Some(delta) = row.ram_delta_vs_previous {
            cumulative_ram_delta += delta;
            if worst_commit_by_ram.as_ref().map(|item: &WorstCommitSummary| delta > item.delta).unwrap_or(true) {
                worst_commit_by_ram = Some(WorstCommitSummary {
                    commit: row.short_commit.clone(),
                    delta,
                    subject: row.subject.clone(),
                    date: row.commit_time.clone(),
                });
            }
        }
    }
    let first_rule_violation = rows.iter().find(|row| row.rule_violations_count > 0).map(|row| FirstRuleViolationSummary {
        commit: row.short_commit.clone(),
        rule_ids: load_rule_ids_for_build(db_path, analyzed_builds.iter().find(|build| build.git.as_ref().map(|git| git.commit_hash.as_str()) == Some(row.commit.as_str())).map(|b| b.id).unwrap_or_default()).unwrap_or_default(),
        subject: row.subject.clone(),
    });
    let range_metrics = match (analyzed_builds.first(), analyzed_builds.last()) {
        (Some(first), Some(last)) if first.id != last.id => Some(build_metric_diff(db_path, last.id, first.id)?),
        _ => None,
    };
    let changed_files_summary = if include_changed_files {
        Some(build_changed_files_summary(
            repo,
            &resolved.diff_base,
            &resolved.diff_head,
            rows.last()
                .map(|_| {
                    range_metrics
                        .as_ref()
                        .map(|diff| diff.source_files.iter().map(|item| item.name.clone()).collect::<Vec<_>>())
                        .unwrap_or_default()
                })
                .unwrap_or_default(),
        )?)
    } else {
        None
    };
    Ok(RangeDiffReport {
        repo_id: repo_root,
        input_range_spec: spec.to_string(),
        comparison_mode: resolved.mode,
        resolved_base: resolved.resolved_base,
        resolved_head: resolved.resolved_head,
        resolved_merge_base: resolved.resolved_merge_base,
        order: commit_order_name(order).to_string(),
        total_commits_in_git_range: commits.len(),
        analyzed_commits_count: rows.len(),
        missing_analysis_commits_count: commits.len().saturating_sub(rows.len()),
        first_analyzed_commit: rows.first().map(|row| row.short_commit.clone()),
        last_analyzed_commit: rows.last().map(|row| row.short_commit.clone()),
        cumulative_rom_delta,
        cumulative_ram_delta,
        worst_commit_by_rom,
        worst_commit_by_ram,
        first_rule_violation,
        top_changed_sections: range_metrics.as_ref().map(|diff| diff.sections.clone()).unwrap_or_default(),
        top_changed_objects: range_metrics.as_ref().map(|diff| diff.objects.clone()).unwrap_or_default(),
        top_changed_source_files: range_metrics.as_ref().map(|diff| diff.source_files.clone()).unwrap_or_default(),
        top_changed_symbols: range_metrics.as_ref().map(|diff| diff.symbols.clone()).unwrap_or_default(),
        top_changed_rust_dependencies: range_metrics
            .as_ref()
            .map(|diff| diff.rust_dependencies.clone())
            .unwrap_or_default(),
        top_changed_rust_families: range_metrics
            .as_ref()
            .map(|diff| diff.rust_families.clone())
            .unwrap_or_default(),
        changed_files_summary,
        timeline_rows: rows,
    })
}

pub fn regression_origin(
    db_path: &Path,
    repo: Option<&Path>,
    spec: &str,
    detector_type: RegressionDetector,
    key: &str,
    mode: RegressionMode,
    threshold: Option<i64>,
    threshold_percent: Option<f64>,
    jump_threshold: Option<i64>,
    order: CommitOrder,
    include_evidence: bool,
    include_changed_files: bool,
    bisect_like: bool,
    max_steps: usize,
    limit_commits: Option<usize>,
    profile: Option<&str>,
    toolchain: Option<&str>,
    target: Option<&str>,
) -> Result<RegressionReport, String> {
    validate_regression_query(detector_type.clone(), key, mode, threshold, threshold_percent, jump_threshold)?;
    let repo_root = resolve_repo_root(repo).ok_or_else(|| "git repository was not found".to_string())?;
    let resolved = resolve_range_spec(repo, spec)?;
    let mut commits = list_commits(repo, &resolved.diff_base, 1, CommitOrder::Timestamp)?;
    let mut range_commits = list_range_commits(repo, &resolved.git_range, order)?;
    range_commits.reverse();
    commits.extend(range_commits);
    if let Some(limit) = limit_commits {
        commits.truncate(limit);
    }
    let all_builds = list_builds(db_path)?;
    let build_map = latest_build_by_commit(&all_builds, &repo_root);
    let mut analyzed = Vec::new();
    for (index, commit) in commits.iter().enumerate() {
        let Some(build) = build_map.get(&commit.commit).cloned() else {
            continue;
        };
        if !matches_filters(&build, profile, toolchain, target, None) {
            continue;
        }
        analyzed.push(RegressionPoint {
            commit_index: index,
            commit: commit.clone(),
            build,
        });
    }
    let mixed_configuration = analyzed
        .iter()
        .map(|point| variant_key(&point.build))
        .collect::<BTreeSet<_>>()
        .len()
        > 1;
    let detection = match detector_type {
        RegressionDetector::Metric => detect_metric_regression(
            db_path,
            &analyzed,
            key,
            mode,
            threshold,
            threshold_percent,
            jump_threshold,
        )?,
        RegressionDetector::Rule => detect_rule_regression(db_path, &analyzed, key)?,
        RegressionDetector::Entity => detect_entity_regression(db_path, &analyzed, key)?,
    };
    let missing_analysis_count = commits.len().saturating_sub(analyzed.len());
    let missing_between_boundary = detection
        .last_good_index
        .zip(detection.first_bad_index)
        .map(|(good, bad)| {
            let commit_gap = analyzed[bad].commit_index.saturating_sub(analyzed[good].commit_index + 1);
            commit_gap
        })
        .unwrap_or(missing_analysis_count);
    let confidence = classify_regression_confidence(
        analyzed.is_empty(),
        detection.last_good_index,
        detection.first_bad_index,
        missing_between_boundary,
        missing_analysis_count,
        mixed_configuration,
    );
    let reasoning = build_regression_reasoning(&detector_type, key, mode, &detection, confidence, missing_between_boundary);
    let origin = RegressionOrigin {
        last_good: detection.last_good_index.map(|index| regression_origin_point(&analyzed[index], detection.values[index])),
        first_observed_bad: detection.first_bad_index.map(|index| regression_origin_point(&analyzed[index], detection.values[index])),
        first_bad_candidate: detection.first_bad_index.map(|index| regression_origin_point(&analyzed[index], detection.values[index])),
    };
    let evidence = if include_evidence {
        let transition_window = build_transition_window(&analyzed, &detection);
        let top_growth = detection
            .last_good_index
            .zip(detection.first_bad_index)
            .map(|(good, bad)| build_metric_diff(db_path, analyzed[bad].build.id, analyzed[good].build.id))
            .transpose()?
            .map(|diff| TimelineTopIncreases {
                sections: diff.sections,
                objects: diff.objects,
                source_files: diff.source_files,
                symbols: diff.symbols,
            })
            .unwrap_or_default();
        let changed_files = if include_changed_files {
            detection.last_good_index.zip(detection.first_bad_index).map(|(good, bad)| {
                build_changed_files_summary(
                    repo,
                    &analyzed[good].commit.commit,
                    &analyzed[bad].commit.commit,
                    top_growth.source_files.iter().map(|item| item.name.clone()).collect::<Vec<_>>(),
                )
            }).transpose()?
        } else {
            None
        };
        let related_rule_hits = if matches!(detector_type, RegressionDetector::Rule) {
            detection
                .first_bad_index
                .map(|index| load_rule_ids_for_build(db_path, analyzed[index].build.id))
                .transpose()?
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let narrowed_commits = if bisect_like {
            build_narrowed_commits(&analyzed, &detection, max_steps)
        } else {
            Vec::new()
        };
        Some(RegressionEvidence {
            transition_window,
            top_growth,
            changed_files,
            related_rule_hits,
            narrowed_commits,
        })
    } else {
        None
    };
    Ok(RegressionReport {
        repo_id: repo_root,
        query: RegressionQuery {
            detector_type,
            key: key.to_string(),
            mode,
            range_spec: spec.to_string(),
            resolved_base: resolved.resolved_base,
            resolved_head: resolved.resolved_head,
            order: commit_order_name(order).to_string(),
            threshold,
            threshold_percent,
            jump_threshold,
            include_evidence,
            include_changed_files,
            bisect_like,
        },
        summary: RegressionSummary {
            searched_commit_count: commits.len(),
            analyzed_commit_count: analyzed.len(),
            missing_analysis_count,
            confidence,
            mixed_configuration,
            reasoning,
        },
        origin,
        evidence,
    })
}

fn detect_metric_regression(
    db_path: &Path,
    analyzed: &[RegressionPoint],
    key: &str,
    mode: RegressionMode,
    threshold: Option<i64>,
    threshold_percent: Option<f64>,
    jump_threshold: Option<i64>,
) -> Result<DetectionResult, String> {
    let conn = open_history_db(db_path)?;
    init_schema(&conn)?;
    let mut values = Vec::with_capacity(analyzed.len());
    for point in analyzed {
        values.push(load_metric_value(&conn, point.build.id, key)?);
    }
    let mut result = DetectionResult {
        values,
        ..DetectionResult::default()
    };
    if analyzed.is_empty() {
        return Ok(result);
    }
    match mode {
        RegressionMode::FirstCrossing => {
            let Some(baseline) = result.values.first().and_then(|value| *value) else {
                return Ok(result);
            };
            let absolute_threshold = threshold_percent
                .map(|percent| ((baseline as f64) * percent / 100.0).round() as i64)
                .or(threshold)
                .unwrap_or_default();
            result.last_good_index = Some(0);
            for index in 1..result.values.len() {
                let Some(value) = result.values[index] else {
                    continue;
                };
                let delta = value - baseline;
                if delta >= absolute_threshold {
                    result.first_bad_index = Some(index);
                    break;
                }
                result.last_good_index = Some(index);
            }
        }
        RegressionMode::FirstJump => {
            let threshold = jump_threshold.unwrap_or_default();
            let mut previous_seen = None;
            for index in 0..result.values.len() {
                let Some(value) = result.values[index] else {
                    continue;
                };
                if let Some((previous_index, previous_value)) = previous_seen {
                    if value - previous_value >= threshold {
                        result.last_good_index = Some(previous_index);
                        result.first_bad_index = Some(index);
                        break;
                    }
                }
                previous_seen = Some((index, value));
            }
            if result.first_bad_index.is_none() {
                result.last_good_index = previous_seen.map(|(index, _)| index);
            }
        }
        _ => {}
    }
    Ok(result)
}

fn detect_rule_regression(db_path: &Path, analyzed: &[RegressionPoint], key: &str) -> Result<DetectionResult, String> {
    let mut result = DetectionResult {
        values: vec![None; analyzed.len()],
        ..DetectionResult::default()
    };
    for (index, point) in analyzed.iter().enumerate() {
        let rule_ids = load_rule_ids_for_build(db_path, point.build.id)?;
        if rule_ids.iter().any(|item| item == key) {
            result.first_bad_index = Some(index);
            break;
        }
        result.last_good_index = Some(index);
    }
    Ok(result)
}

fn detect_entity_regression(db_path: &Path, analyzed: &[RegressionPoint], key: &str) -> Result<DetectionResult, String> {
    let conn = open_history_db(db_path)?;
    init_schema(&conn)?;
    let mut values = Vec::with_capacity(analyzed.len());
    let mut result = DetectionResult::default();
    for (index, point) in analyzed.iter().enumerate() {
        let value = load_entity_value(&conn, point.build.id, key)?;
        values.push(value);
        if result.first_bad_index.is_none() && value.unwrap_or_default() > 0 {
            result.first_bad_index = Some(index);
            break;
        }
        result.last_good_index = Some(index);
    }
    while values.len() < analyzed.len() {
        values.push(None);
    }
    result.values = values;
    Ok(result)
}

fn load_metric_value(conn: &Connection, build_id: i64, key: &str) -> Result<Option<i64>, String> {
    if key == "rom_total" {
        return conn
            .query_row("SELECT rom_bytes FROM builds WHERE id = ?1", params![build_id], |row| row.get::<_, i64>(0))
            .optional()
            .map_err(|err| format!("failed to query rom_total: {err}"));
    }
    if key == "ram_total" {
        return conn
            .query_row("SELECT ram_bytes FROM builds WHERE id = ?1", params![build_id], |row| row.get::<_, i64>(0))
            .optional()
            .map_err(|err| format!("failed to query ram_total: {err}"));
    }
    if let Some(name) = key.strip_prefix("region:").and_then(|value| value.strip_suffix(".used")) {
        return query_named_metric_value(conn, "region_metrics", "region_name", "used_bytes", build_id, name);
    }
    if let Some(name) = key.strip_prefix("section:").and_then(|value| value.strip_suffix(".size")) {
        return query_named_metric_value(conn, "section_metrics", "section_name", "size_bytes", build_id, name);
    }
    if let Some(name) = key.strip_prefix("source:").and_then(|value| value.strip_suffix(".size")) {
        return query_named_metric_value(conn, "source_file_metrics", "path", "size_bytes", build_id, name);
    }
    if let Some(name) = key.strip_prefix("object:").and_then(|value| value.strip_suffix(".size")) {
        return query_named_metric_value(conn, "object_metrics", "object_path", "size_bytes", build_id, name);
    }
    if let Some(name) = key.strip_prefix("symbol:").and_then(|value| value.strip_suffix(".size")) {
        return query_named_metric_value(conn, "symbol_metrics", "name", "size_bytes", build_id, name);
    }
    if let Some(name) = key.strip_prefix("rust-package:").and_then(|value| value.strip_suffix(".size")) {
        return query_rust_metric_value(conn, build_id, "package", name);
    }
    if let Some(name) = key.strip_prefix("rust-target:").and_then(|value| value.strip_suffix(".size")) {
        return query_rust_metric_value(conn, build_id, "target", name);
    }
    if let Some(name) = key.strip_prefix("rust-crate:").and_then(|value| value.strip_suffix(".size")) {
        return query_rust_metric_value(conn, build_id, "crate", name);
    }
    if let Some(name) = key.strip_prefix("rust-dependency:").and_then(|value| value.strip_suffix(".size")) {
        return query_rust_metric_value(conn, build_id, "dependency", name);
    }
    if let Some(name) = key.strip_prefix("rust-source:").and_then(|value| value.strip_suffix(".size")) {
        return query_rust_metric_value(conn, build_id, "source", name);
    }
    if let Some(name) = key.strip_prefix("rust-family:").and_then(|value| value.strip_suffix(".size")) {
        return query_rust_metric_value_like(conn, build_id, "family:%", name);
    }
    Err(format!("unsupported metric key '{key}'"))
}

fn load_entity_value(conn: &Connection, build_id: i64, key: &str) -> Result<Option<i64>, String> {
    if let Some(name) = key.strip_prefix("symbol:") {
        return query_named_metric_value(conn, "symbol_metrics", "name", "size_bytes", build_id, name);
    }
    if let Some(name) = key.strip_prefix("object:") {
        return query_named_metric_value(conn, "object_metrics", "object_path", "size_bytes", build_id, name);
    }
    if let Some(name) = key.strip_prefix("source:") {
        return query_named_metric_value(conn, "source_file_metrics", "path", "size_bytes", build_id, name);
    }
    if let Some(name) = key.strip_prefix("section:") {
        return query_named_metric_value(conn, "section_metrics", "section_name", "size_bytes", build_id, name);
    }
    if let Some(name) = key.strip_prefix("region:") {
        return query_named_metric_value(conn, "region_metrics", "region_name", "used_bytes", build_id, name);
    }
    if let Some(name) = key.strip_prefix("rust-package:") {
        return query_rust_metric_value(conn, build_id, "package", name);
    }
    if let Some(name) = key.strip_prefix("rust-target:") {
        return query_rust_metric_value(conn, build_id, "target", name);
    }
    if let Some(name) = key.strip_prefix("rust-crate:") {
        return query_rust_metric_value(conn, build_id, "crate", name);
    }
    if let Some(name) = key.strip_prefix("rust-dependency:") {
        return query_rust_metric_value(conn, build_id, "dependency", name);
    }
    if let Some(name) = key.strip_prefix("rust-source:") {
        return query_rust_metric_value(conn, build_id, "source", name);
    }
    if let Some(name) = key.strip_prefix("rust-family:") {
        return query_rust_metric_value_like(conn, build_id, "family:%", name);
    }
    Err(format!("unsupported entity key '{key}'"))
}

fn query_named_metric_value(
    conn: &Connection,
    table: &str,
    name_column: &str,
    value_column: &str,
    build_id: i64,
    name: &str,
) -> Result<Option<i64>, String> {
    let sql = format!(
        "SELECT {value_column} FROM {table} WHERE build_id = ?1 AND {name_column} = ?2 LIMIT 1"
    );
    conn.query_row(&sql, params![build_id, name], |row| row.get::<_, i64>(0))
        .optional()
        .map_err(|err| format!("failed to query {table} '{name}': {err}"))
}

fn query_rust_metric_value(conn: &Connection, build_id: i64, scope: &str, name: &str) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT size_bytes FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope = ?2 AND name = ?3 LIMIT 1",
        params![build_id, scope, name],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map_err(|err| format!("failed to query rust_aggregate_metrics '{scope}:{name}': {err}"))
}

fn query_rust_metric_value_like(conn: &Connection, build_id: i64, scope_pattern: &str, name: &str) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT SUM(size_bytes) FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope LIKE ?2 AND name = ?3 GROUP BY name LIMIT 1",
        params![build_id, scope_pattern, name],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map_err(|err| format!("failed to query rust_aggregate_metrics '{scope_pattern}:{name}': {err}"))
}

fn classify_regression_confidence(
    no_analyzed: bool,
    last_good_index: Option<usize>,
    first_bad_index: Option<usize>,
    missing_between_boundary: usize,
    missing_total: usize,
    mixed_configuration: bool,
) -> RegressionConfidence {
    if no_analyzed {
        return RegressionConfidence::Unknown;
    }
    let Some(first_bad_index) = first_bad_index else {
        return RegressionConfidence::Unknown;
    };
    if mixed_configuration {
        return RegressionConfidence::Low;
    }
    let Some(last_good_index) = last_good_index else {
        return if first_bad_index == 0 {
            RegressionConfidence::Low
        } else {
            RegressionConfidence::Medium
        };
    };
    if missing_between_boundary == 0 && first_bad_index == last_good_index + 1 {
        return RegressionConfidence::High;
    }
    if missing_total > 0 {
        return RegressionConfidence::Medium;
    }
    RegressionConfidence::Low
}

fn build_regression_reasoning(
    detector_type: &RegressionDetector,
    key: &str,
    mode: RegressionMode,
    detection: &DetectionResult,
    confidence: RegressionConfidence,
    missing_between_boundary: usize,
) -> String {
    let Some(first_bad_index) = detection.first_bad_index else {
        return format!(
            "No analyzed commit matched detector {:?} for '{}' in the requested range.",
            detector_type, key
        );
    };
    let first_bad = detection.values.get(first_bad_index).and_then(|value| *value);
    let boundary = detection
        .last_good_index
        .map(|index| format!("between analyzed commits {index} and {first_bad_index}"))
        .unwrap_or_else(|| format!("at the first analyzed bad commit index {first_bad_index}"));
    let detector_label = match detector_type {
        RegressionDetector::Metric => match mode {
            RegressionMode::FirstCrossing => "first exceeded threshold",
            RegressionMode::FirstJump => "first crossed jump threshold",
            _ => "matched",
        },
        RegressionDetector::Rule => "first became active",
        RegressionDetector::Entity => "was first observed",
    };
    let mut reasoning = format!("{} {} for '{}' {}; ", format!("{:?}", detector_type), detector_label, key, boundary);
    if let Some(value) = first_bad {
        reasoning.push_str(&format!("first observed value is {value}. "));
    }
    if missing_between_boundary > 0 {
        reasoning.push_str(&format!(
            "{} commits in the boundary have no stored analysis, so confidence is {:?}.",
            missing_between_boundary, confidence
        ));
    } else {
        reasoning.push_str(&format!("No missing analyzed commits around the boundary; confidence is {:?}.", confidence));
    }
    reasoning
}

fn regression_origin_point(point: &RegressionPoint, value: Option<i64>) -> RegressionOriginPoint {
    RegressionOriginPoint {
        commit: point.commit.commit.clone(),
        short_commit: point.commit.short_commit.clone(),
        subject: point.commit.subject.clone(),
        value,
    }
}

fn build_transition_window(analyzed: &[RegressionPoint], detection: &DetectionResult) -> Vec<RegressionWindowRow> {
    let mut rows = Vec::new();
    if let Some(index) = detection.last_good_index {
        rows.push(RegressionWindowRow {
            commit: analyzed[index].commit.commit.clone(),
            short_commit: analyzed[index].commit.short_commit.clone(),
            subject: analyzed[index].commit.subject.clone(),
            status: "good".to_string(),
            value: detection.values[index],
        });
    }
    if let Some(index) = detection.first_bad_index {
        rows.push(RegressionWindowRow {
            commit: analyzed[index].commit.commit.clone(),
            short_commit: analyzed[index].commit.short_commit.clone(),
            subject: analyzed[index].commit.subject.clone(),
            status: "bad".to_string(),
            value: detection.values[index],
        });
    }
    rows
}

fn build_narrowed_commits(analyzed: &[RegressionPoint], detection: &DetectionResult, max_steps: usize) -> Vec<String> {
    let Some(mut left) = detection.last_good_index else {
        return Vec::new();
    };
    let Some(right) = detection.first_bad_index else {
        return Vec::new();
    };
    let mut rows = Vec::new();
    let mut steps = 0usize;
    while right > left + 1 && steps < max_steps {
        let mid = left + (right - left) / 2;
        rows.push(analyzed[mid].commit.short_commit.clone());
        left = mid;
        steps += 1;
    }
    rows
}

fn validate_regression_query(
    detector_type: RegressionDetector,
    key: &str,
    mode: RegressionMode,
    threshold: Option<i64>,
    threshold_percent: Option<f64>,
    jump_threshold: Option<i64>,
) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("regression detector key must not be empty".to_string());
    }
    match detector_type {
        RegressionDetector::Metric => match mode {
            RegressionMode::FirstCrossing => {
                if threshold.is_some() && threshold_percent.is_some() {
                    return Err("use either --threshold or --threshold-percent, not both".to_string());
                }
                if threshold.is_none() && threshold_percent.is_none() {
                    return Err("metric first-crossing requires --threshold or --threshold-percent".to_string());
                }
            }
            RegressionMode::FirstJump => {
                if jump_threshold.is_none() {
                    return Err("metric first-jump requires --jump-threshold".to_string());
                }
            }
            _ => return Err("metric detector supports only first-crossing or first-jump".to_string()),
        },
        RegressionDetector::Rule => {
            if !matches!(mode, RegressionMode::FirstViolation) {
                return Err("rule detector supports only first-violation".to_string());
            }
        }
        RegressionDetector::Entity => {
            if !matches!(mode, RegressionMode::FirstPresence) {
                return Err("entity detector supports only first-presence".to_string());
            }
        }
    }
    Ok(())
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
                note: None,
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
    let sql = match mode {
        NamedMetricMode::ByName => format!(
            "SELECT b.id, b.created_at, t.{value_column}
             FROM builds b
             JOIN {table} t ON t.build_id = b.id
             WHERE t.{name_column} = ?1
             ORDER BY b.id DESC LIMIT ?2"
        ),
        NamedMetricMode::ByNameAndScope(_) => format!(
            "SELECT b.id, b.created_at, t.{value_column}
             FROM builds b
             JOIN {table} t ON t.build_id = b.id
             WHERE t.{name_column} = ?1 AND t.scope = ?2
             ORDER BY b.id DESC LIMIT ?3"
        ),
    };
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
                    note: None,
                })
            })
            .map_err(|err| format!("failed to query named trend metric: {err}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to collect named trend metric: {err}"))?,
        NamedMetricMode::ByNameAndScope(scope) => stmt
            .query_map(params![name, scope, last as i64], |row| {
                Ok(TrendPoint {
                    build_id: row.get(0)?,
                    created_at: row.get(1)?,
                    label: name.to_string(),
                    value: row.get::<_, i64>(2)?,
                    format,
                    note: Some(scope.to_string()),
                })
            })
            .map_err(|err| format!("failed to query named scoped trend metric: {err}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to collect named scoped trend metric: {err}"))?,
    };
    points.reverse();
    Ok(points)
}

fn query_named_metric_like_scope(conn: &Connection, name: &str, scope_pattern: &str, last: usize) -> Result<Vec<TrendPoint>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.created_at, COALESCE(SUM(t.size_bytes), 0)
             FROM builds b
             JOIN rust_aggregate_metrics t ON t.build_id = b.id
             WHERE t.name = ?1 AND t.scope LIKE ?2
             GROUP BY b.id, b.created_at
             ORDER BY b.id DESC LIMIT ?3",
        )
        .map_err(|err| format!("failed to prepare Rust family trend query: {err}"))?;
    let rows = stmt
        .query_map(params![name, scope_pattern, last as i64], |row| {
            Ok(TrendPoint {
                build_id: row.get(0)?,
                created_at: row.get(1)?,
                label: name.to_string(),
                value: row.get::<_, i64>(2)?,
                format: TrendFormat::Bytes,
                note: Some("family".to_string()),
            })
        })
        .map_err(|err| format!("failed to query Rust family trend metric: {err}"))?;
    let mut points = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect Rust family trend metric: {err}"))?;
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
                note: None,
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
                note: None,
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
            "#{} {} ROM={} RAM={} warnings={} errors={} {} [{} / {}]{}",
            item.id,
            item.created_at,
            item.rom_bytes,
            item.ram_bytes,
            item.warning_count,
            item.error_count,
            item.elf_path,
            item.linker_family,
            item.map_format,
            format_git_summary(item.git.as_ref())
        );
        if let Some(rust) = item.rust_context.as_ref() {
            println!(
                "    rust={}{}{}",
                rust.package_name.as_deref().unwrap_or("-"),
                rust.target_name
                    .as_deref()
                    .map(|value| format!(" target={value}"))
                    .unwrap_or_default(),
                rust.profile
                    .as_deref()
                    .map(|value| format!(" profile={value}"))
                    .unwrap_or_default()
            );
        }
    }
}

pub fn print_build_detail(detail: &BuildDetail, view: crate::model::ViewMode) {
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
    if let Some(git) = detail.build.git.as_ref() {
        println!(
            "Git: {}{}{}{}",
            git.short_commit_hash,
            git.branch_name
                .as_deref()
                .map(|value| format!(" | branch={value}"))
                .unwrap_or_else(|| " | detached".to_string()),
            git.describe
                .as_deref()
                .map(|value| format!(" | describe={value}"))
                .unwrap_or_default(),
            if git.is_dirty { " | dirty" } else { "" }
        );
        if let Some(subject) = git.commit_subject.as_deref() {
            println!("Git subject: {subject}");
        }
    }
    if let Some(rust) = detail.build.rust_context.as_ref() {
        println!(
            "Rust: {}{}{}{}{}",
            rust.package_name.as_deref().unwrap_or("-"),
            rust.target_name
                .as_deref()
                .map(|value| format!(" | target={value}"))
                .unwrap_or_default(),
            (!rust.target_kind.is_empty())
                .then(|| format!(" | kind={}", rust.target_kind.join(",")))
                .unwrap_or_default(),
            rust.profile
                .as_deref()
                .map(|value| format!(" | profile={value}"))
                .unwrap_or_default(),
            rust.target_triple
                .as_deref()
                .map(|value| format!(" | triple={value}"))
                .unwrap_or_default()
        );
    }
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
    if matches!(view, crate::model::ViewMode::Rust) {
        if !detail.rust_packages.is_empty() {
            println!("Rust packages:");
            for (name, size, symbols) in &detail.rust_packages {
                println!("  {} {} symbols={}", name, size, symbols);
            }
        }
        if !detail.rust_targets.is_empty() {
            println!("Rust targets:");
            for (name, size, symbols) in &detail.rust_targets {
                println!("  {} {} symbols={}", name, size, symbols);
            }
        }
        if !detail.rust_crates.is_empty() {
            println!("Rust crates:");
            for (name, size, symbols) in &detail.rust_crates {
                println!("  {} {} symbols={}", name, size, symbols);
            }
        }
        if !detail.rust_dependencies.is_empty() {
            println!("Rust dependency crates:");
            for (name, size, symbols) in &detail.rust_dependencies {
                println!("  {} {} symbols={}", name, size, symbols);
            }
        }
        if !detail.rust_source_files.is_empty() {
            println!("Rust source files:");
            for (name, size, symbols) in &detail.rust_source_files {
                println!("  {} {} symbols={}", name, size, symbols);
            }
        }
        if !detail.rust_families.is_empty() {
            println!("Rust families:");
            for (name, size, symbols) in &detail.rust_families {
                println!("  {} {} symbols={}", name, size, symbols);
            }
        }
    }
    if !detail.why_linked.is_empty() {
        println!("Why linked:");
        for item in &detail.why_linked {
            println!(
                "  {} [{} {}] {} ({})",
                item.target, item.kind, item.confidence, item.summary, item.current_size
            );
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
            TrendFormat::Bytes => println!(
                "#{} {} {}={}{}",
                point.build_id,
                point.created_at,
                point.label,
                point.value,
                point.note.as_deref().map(|item| format!(" | {}", item)).unwrap_or_default()
            ),
            TrendFormat::Count => println!(
                "#{} {} {}={}{}",
                point.build_id,
                point.created_at,
                point.label,
                point.value,
                point.note.as_deref().map(|item| format!(" | {}", item)).unwrap_or_default()
            ),
            TrendFormat::Percent => println!(
                "#{} {} {}={:.1}%{}",
                point.build_id,
                point.created_at,
                point.label,
                point.value as f64 / 10.0,
                point.note.as_deref().map(|item| format!(" | {}", item)).unwrap_or_default()
            ),
        }
    }
}

pub fn print_commit_timeline(report: &CommitTimelineReport, view: crate::model::ViewMode) {
    if report.rows.is_empty() {
        println!("No analyzed commits matched the requested timeline.");
        return;
    }
    for row in &report.rows {
        let rom_delta = row
            .rom_delta_vs_previous
            .map(|delta| format!(" | ROM delta {delta:+}"))
            .unwrap_or_default();
        let ram_delta = row
            .ram_delta_vs_previous
            .map(|delta| format!(" | RAM delta {delta:+}"))
            .unwrap_or_default();
        println!(
            "{} {} ROM={} RAM={} warnings={}{}{}",
            row.short_commit,
            row.subject,
            row.rom_total,
            row.ram_total,
            row.rule_violations_count,
            rom_delta,
            ram_delta
        );
        if matches!(view, crate::model::ViewMode::Rust) {
            if let Some(item) = row.top_increases.source_files.first() {
                println!("    top source delta: {} ({:+})", item.name, item.delta);
            }
        }
    }
}

pub fn print_range_diff(report: &RangeDiffReport, view: crate::model::ViewMode) {
    println!(
        "Range {} {} analyzed={} missing={} cumulative_rom={:+} cumulative_ram={:+}",
        report.input_range_spec,
        report.comparison_mode,
        report.analyzed_commits_count,
        report.missing_analysis_commits_count,
        report.cumulative_rom_delta,
        report.cumulative_ram_delta
    );
    if let Some(item) = report.worst_commit_by_rom.as_ref() {
        println!("Worst ROM commit: {} {} ({:+})", item.commit, item.subject, item.delta);
    }
    if let Some(item) = report.worst_commit_by_ram.as_ref() {
        println!("Worst RAM commit: {} {} ({:+})", item.commit, item.subject, item.delta);
    }
    if matches!(view, crate::model::ViewMode::Rust) {
        if let Some(item) = report.top_changed_rust_dependencies.first() {
            println!("Top Rust dependency delta: {} ({:+})", item.name, item.delta);
        }
        if let Some(item) = report.top_changed_rust_families.first() {
            println!("Top Rust family delta: {} ({:+})", item.name, item.delta);
        }
    }
}

pub fn print_regression_report(report: &RegressionReport) {
    println!(
        "Regression {} {} {} analyzed={} missing={} confidence={:?}",
        match report.query.detector_type {
            RegressionDetector::Metric => "metric",
            RegressionDetector::Rule => "rule",
            RegressionDetector::Entity => "entity",
        },
        report.query.key,
        report.query.range_spec,
        report.summary.analyzed_commit_count,
        report.summary.missing_analysis_count,
        report.summary.confidence
    );
    println!("{}", report.summary.reasoning);
    if let Some(point) = report.origin.last_good.as_ref() {
        println!(
            "Last good: {} {}{}",
            point.short_commit,
            point.subject,
            point.value.map(|value| format!(" value={value}")).unwrap_or_default()
        );
    }
    if let Some(point) = report.origin.first_observed_bad.as_ref() {
        println!(
            "First observed bad: {} {}{}",
            point.short_commit,
            point.subject,
            point.value.map(|value| format!(" value={value}")).unwrap_or_default()
        );
    }
}

pub fn write_commit_timeline_html(path: &Path, report: &CommitTimelineReport) -> Result<(), String> {
    let filter_summary = [
        report
            .filters
            .branch
            .as_deref()
            .map(|value| format!("branch {value}")),
        report
            .filters
            .profile
            .as_deref()
            .map(|value| format!("profile {value}")),
        report
            .filters
            .toolchain
            .as_deref()
            .map(|value| format!("toolchain {value}")),
        report
            .filters
            .target
            .as_deref()
            .map(|value| format!("target {value}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" | ");
    let rows = report
        .rows
        .iter()
        .map(|row| {
            let rom_delta = render_delta_badge(row.rom_delta_vs_previous);
            let ram_delta = render_delta_badge(row.ram_delta_vs_previous);
            format!(
                "<tr><td><code>{}</code></td><td class=\"muted\">{}</td><td class=\"subject\">{}</td><td class=\"mono\">{}</td><td class=\"mono\">{}</td><td>{}</td><td>{}</td><td><span class=\"metric-pill\">{}</span></td></tr>",
                escape_html(&row.short_commit),
                escape_html(&row.commit_time),
                escape_html(&row.subject),
                row.rom_total,
                row.ram_total,
                rom_delta,
                ram_delta,
                row.rule_violations_count
            )
        })
        .collect::<Vec<_>>()
        .join("");
    fs::write(
        path,
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>fwmap history commits</title><style>{}</style></head><body><main class=\"page\"><section class=\"hero\"><p class=\"eyebrow\">fwmap history</p><h1>Commit Timeline</h1><p class=\"lede\">Analyzed commits aligned to Git history with ROM/RAM deltas against the next older analyzed build.</p></section><section class=\"summary-grid\"><article class=\"summary-card\"><span class=\"label\">Repository</span><strong>{}</strong></article><article class=\"summary-card\"><span class=\"label\">Rows</span><strong>{}</strong></article><article class=\"summary-card\"><span class=\"label\">Order</span><strong>{}</strong></article><article class=\"summary-card\"><span class=\"label\">Filters</span><strong>{}</strong></article></section><section class=\"panel\"><div class=\"table-wrap\"><table><thead><tr><th>Commit</th><th>Time</th><th>Subject</th><th>ROM</th><th>RAM</th><th>ROM delta</th><th>RAM delta</th><th>Rules</th></tr></thead><tbody>{}</tbody></table></div></section></main></body></html>",
            history_report_css(),
            escape_html(&report.repo_id),
            report.rows.len(),
            escape_html(&report.order),
            if filter_summary.is_empty() {
                "none".to_string()
            } else {
                escape_html(&filter_summary)
            },
            rows
        ),
    )
    .map_err(|err| format!("failed to write commit timeline HTML '{}': {err}", path.display()))
}

pub fn write_range_diff_html(path: &Path, report: &RangeDiffReport) -> Result<(), String> {
    let worst_rom = report
        .worst_commit_by_rom
        .as_ref()
        .map(|item| format!("{} ({:+})", item.commit, item.delta))
        .unwrap_or_else(|| "-".to_string());
    let worst_ram = report
        .worst_commit_by_ram
        .as_ref()
        .map(|item| format!("{} ({:+})", item.commit, item.delta))
        .unwrap_or_else(|| "-".to_string());
    let rows = report
        .timeline_rows
        .iter()
        .map(|row| {
            let rom_delta = render_delta_badge(row.rom_delta_vs_previous);
            let ram_delta = render_delta_badge(row.ram_delta_vs_previous);
            format!(
                "<tr><td><code>{}</code></td><td class=\"subject\">{}</td><td>{}</td><td>{}</td><td><span class=\"metric-pill\">{}</span></td></tr>",
                escape_html(&row.short_commit),
                escape_html(&row.subject),
                rom_delta,
                ram_delta,
                row.rule_violations_count
            )
        })
        .collect::<Vec<_>>()
        .join("");
    fs::write(
        path,
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>fwmap history range</title><style>{}</style></head><body><main class=\"page\"><section class=\"hero\"><p class=\"eyebrow\">fwmap history</p><h1>Range Diff</h1><p class=\"lede\">Summary for <code>{}</code> with cumulative deltas and per-commit changes across the analyzed range.</p></section><section class=\"summary-grid\"><article class=\"summary-card\"><span class=\"label\">Analyzed commits</span><strong>{}</strong></article><article class=\"summary-card\"><span class=\"label\">Missing analysis</span><strong>{}</strong></article><article class=\"summary-card\"><span class=\"label\">Cumulative ROM</span><strong>{}</strong></article><article class=\"summary-card\"><span class=\"label\">Cumulative RAM</span><strong>{}</strong></article><article class=\"summary-card\"><span class=\"label\">Worst ROM commit</span><strong>{}</strong></article><article class=\"summary-card\"><span class=\"label\">Worst RAM commit</span><strong>{}</strong></article></section><section class=\"panel\"><div class=\"table-wrap\"><table><thead><tr><th>Commit</th><th>Subject</th><th>ROM delta</th><th>RAM delta</th><th>Rules</th></tr></thead><tbody>{}</tbody></table></div></section></main></body></html>",
            history_report_css(),
            escape_html(&report.input_range_spec),
            report.analyzed_commits_count,
            report.missing_analysis_commits_count,
            format_signed(report.cumulative_rom_delta),
            format_signed(report.cumulative_ram_delta),
            escape_html(&worst_rom),
            escape_html(&worst_ram),
            rows
        ),
    )
    .map_err(|err| format!("failed to write range diff HTML '{}': {err}", path.display()))
}

pub fn write_regression_html(path: &Path, report: &RegressionReport) -> Result<(), String> {
    let transition_rows = report
        .evidence
        .as_ref()
        .map(|evidence| {
            evidence
                .transition_window
                .iter()
                .map(|row| {
                    format!(
                        "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        escape_html(&row.short_commit),
                        escape_html(&row.status),
                        row.value.map(|value| value.to_string()).unwrap_or_else(|| "-".to_string()),
                        escape_html(&row.subject)
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    fs::write(
        path,
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>fwmap history regression</title></head><body><h1>Regression Origin</h1><p>{}</p><p>confidence={:?} analyzed={} missing={}</p><p>{}</p><table border=\"1\"><thead><tr><th>Commit</th><th>Status</th><th>Value</th><th>Subject</th></tr></thead><tbody>{}</tbody></table></body></html>",
            escape_html(&report.query.range_spec),
            report.summary.confidence,
            report.summary.analyzed_commit_count,
            report.summary.missing_analysis_count,
            escape_html(&report.summary.reasoning),
            transition_rows
        ),
    )
    .map_err(|err| format!("failed to write regression HTML '{}': {err}", path.display()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamedMetricMode {
    ByName,
    ByNameAndScope(&'static str),
}

fn history_report_css() -> &'static str {
    "body{margin:0;font-family:Inter,\"Segoe UI\",system-ui,sans-serif;background:linear-gradient(180deg,#f4f7fb 0%,#eef2f7 100%);color:#17212b}code,.mono{font-family:\"SFMono-Regular\",Consolas,monospace}.page{max-width:1180px;margin:0 auto;padding:32px 20px 56px}.hero{padding:8px 4px 20px}.eyebrow{text-transform:uppercase;letter-spacing:.12em;font-size:12px;color:#6b7a90;margin:0 0 10px}.hero h1{font-size:56px;line-height:1.02;margin:0 0 14px}.lede{max-width:70ch;font-size:18px;line-height:1.6;color:#4b5b70;margin:0}.summary-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:14px;margin:8px 0 22px}.summary-card{background:rgba(255,255,255,.78);backdrop-filter:blur(8px);border:1px solid rgba(132,149,173,.2);border-radius:18px;padding:16px 18px;box-shadow:0 14px 40px rgba(25,42,70,.08)}.summary-card .label{display:block;font-size:12px;letter-spacing:.08em;text-transform:uppercase;color:#6b7a90;margin-bottom:10px}.summary-card strong{display:block;font-size:22px;line-height:1.3;word-break:break-word}.panel{background:rgba(255,255,255,.88);border:1px solid rgba(132,149,173,.2);border-radius:22px;overflow:hidden;box-shadow:0 16px 48px rgba(25,42,70,.08)}.table-wrap{overflow:auto}.table-wrap table{width:100%;border-collapse:separate;border-spacing:0}.table-wrap thead th{position:sticky;top:0;background:#f8fbff;color:#304055;font-size:13px;text-transform:uppercase;letter-spacing:.06em;padding:14px 16px;border-bottom:1px solid #d8e1ec;white-space:nowrap}.table-wrap tbody td{padding:16px;border-bottom:1px solid #e6edf5;vertical-align:top}.table-wrap tbody tr:nth-child(odd){background:rgba(248,251,255,.65)}.table-wrap tbody tr:hover{background:rgba(231,239,249,.95)}.muted{color:#6b7a90;white-space:nowrap}.subject{min-width:320px;line-height:1.45}.delta{display:inline-flex;align-items:center;justify-content:center;min-width:78px;padding:6px 10px;border-radius:999px;font-weight:700;font-size:13px}.delta.pos{background:#e4f6ea;color:#146c43}.delta.neg{background:#fde9e7;color:#b23b2b}.delta.zero,.delta.na{background:#edf2f7;color:#607287}.metric-pill{display:inline-flex;align-items:center;justify-content:center;min-width:34px;padding:5px 9px;border-radius:999px;background:#edf2f7;color:#334155;font-weight:700;font-size:13px}@media (max-width:760px){.page{padding:24px 14px 40px}.hero h1{font-size:38px}.lede{font-size:16px}.subject{min-width:240px}}"
}

fn render_delta_badge(value: Option<i64>) -> String {
    match value {
        Some(delta) if delta > 0 => format!("<span class=\"delta pos\">{}</span>", format_signed(delta)),
        Some(delta) if delta < 0 => format!("<span class=\"delta neg\">{}</span>", format_signed(delta)),
        Some(delta) => format!("<span class=\"delta zero\">{}</span>", format_signed(delta)),
        None => "<span class=\"delta na\">-</span>".to_string(),
    }
}

fn format_signed(value: i64) -> String {
    format!("{value:+}")
}

#[derive(Debug, Clone)]
struct BuildMetricDiff {
    rom_delta: i64,
    ram_delta: i64,
    sections: Vec<ChangeEntry>,
    objects: Vec<ChangeEntry>,
    source_files: Vec<ChangeEntry>,
    symbols: Vec<ChangeEntry>,
    rust_dependencies: Vec<ChangeEntry>,
    rust_families: Vec<ChangeEntry>,
}

#[derive(Debug, Clone)]
struct ResolvedRange {
    mode: String,
    git_range: String,
    resolved_base: String,
    resolved_head: String,
    resolved_merge_base: Option<String>,
    diff_base: String,
    diff_head: String,
}

fn latest_build_by_commit(items: &[BuildRecord], repo_root: &str) -> HashMap<String, BuildRecord> {
    let mut result = HashMap::new();
    for item in items {
        let Some(git) = item.git.as_ref() else {
            continue;
        };
        if git.repo_root != repo_root {
            continue;
        }
        result.entry(git.commit_hash.clone()).or_insert_with(|| item.clone());
    }
    result
}

fn matches_filters(
    build: &BuildRecord,
    profile: Option<&str>,
    toolchain: Option<&str>,
    target: Option<&str>,
    branch: Option<&str>,
) -> bool {
    if let Some(profile) = profile {
        if build.metadata.get("build.profile").map(String::as_str) != Some(profile) {
            return false;
        }
    }
    if let Some(toolchain) = toolchain {
        let value = build
            .metadata
            .get("toolchain.id")
            .map(String::as_str)
            .unwrap_or(build.linker_family.as_str());
        if value != toolchain {
            return false;
        }
    }
    if let Some(target) = target {
        if build.metadata.get("target.id").map(String::as_str) != Some(target) {
            return false;
        }
    }
    if let Some(branch) = branch {
        if build.git.as_ref().and_then(|git| git.branch_name.as_deref()) != Some(branch) {
            return false;
        }
    }
    true
}

fn variant_key(build: &BuildRecord) -> String {
    format!(
        "{}|{}|{}|{}",
        build.metadata.get("build.profile").cloned().unwrap_or_default(),
        build.metadata
            .get("toolchain.id")
            .cloned()
            .unwrap_or_else(|| build.linker_family.clone()),
        build.metadata.get("target.id").cloned().unwrap_or_default(),
        normalized_config_fingerprint(build.metadata.get("config.fingerprint"))
    )
}

fn normalized_config_fingerprint(value: Option<&String>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    value.split('|').take(2).collect::<Vec<_>>().join("|")
}

fn build_timeline_row(repo_id: &str, commit: &GitCommit, build: &BuildRecord, diff: Option<&BuildMetricDiff>) -> CommitTimelineRow {
    let git = build.git.as_ref();
    CommitTimelineRow {
        repo_id: repo_id.to_string(),
        commit: commit.commit.clone(),
        short_commit: commit.short_commit.clone(),
        commit_time: commit.commit_time.clone(),
        author_name: commit.author_name.clone(),
        subject: commit.subject.clone(),
        branch_names: git
            .and_then(|item| item.branch_name.clone())
            .map(|item| vec![item])
            .unwrap_or_default(),
        tag_names: git.map(|item| item.tag_names.clone()).unwrap_or_default(),
        describe: git.and_then(|item| item.describe.clone()),
        build_profile: build.metadata.get("build.profile").cloned(),
        toolchain_id: build
            .metadata
            .get("toolchain.id")
            .cloned()
            .or_else(|| Some(build.linker_family.clone())),
        target_id: build.metadata.get("target.id").cloned(),
        configuration_fingerprint: build.metadata.get("config.fingerprint").cloned(),
        rom_total: build.rom_bytes,
        ram_total: build.ram_bytes,
        rom_delta_vs_previous: diff.map(|item| item.rom_delta),
        ram_delta_vs_previous: diff.map(|item| item.ram_delta),
        rule_violations_count: build.warning_count,
        top_increases: diff
            .map(|item| TimelineTopIncreases {
                sections: item.sections.clone(),
                objects: item.objects.clone(),
                source_files: item.source_files.clone(),
                symbols: item.symbols.clone(),
            })
            .unwrap_or_default(),
    }
}

fn build_metric_diff(db_path: &Path, current_build_id: i64, previous_build_id: i64) -> Result<BuildMetricDiff, String> {
    let conn = open_history_db(db_path)?;
    init_schema(&conn)?;
    let current = load_build_record(&conn, current_build_id)?.ok_or_else(|| format!("build id {current_build_id} was not found"))?;
    let previous = load_build_record(&conn, previous_build_id)?.ok_or_else(|| format!("build id {previous_build_id} was not found"))?;
    Ok(BuildMetricDiff {
        rom_delta: current.rom_bytes as i64 - previous.rom_bytes as i64,
        ram_delta: current.ram_bytes as i64 - previous.ram_bytes as i64,
        sections: diff_metric_entries(
            load_metric_map(&conn, "section_metrics", "section_name", current_build_id)?,
            load_metric_map(&conn, "section_metrics", "section_name", previous_build_id)?,
            3,
        ),
        objects: diff_metric_entries(
            load_metric_map(&conn, "object_metrics", "object_path", current_build_id)?,
            load_metric_map(&conn, "object_metrics", "object_path", previous_build_id)?,
            3,
        ),
        source_files: diff_metric_entries(
            load_metric_map(&conn, "source_file_metrics", "path", current_build_id)?,
            load_metric_map(&conn, "source_file_metrics", "path", previous_build_id)?,
            3,
        ),
        symbols: diff_metric_entries(
            load_metric_map(&conn, "symbol_metrics", "name", current_build_id)?,
            load_metric_map(&conn, "symbol_metrics", "name", previous_build_id)?,
            5,
        ),
        rust_dependencies: diff_metric_entries(
            load_scoped_metric_map(&conn, "dependency", current_build_id)?,
            load_scoped_metric_map(&conn, "dependency", previous_build_id)?,
            3,
        ),
        rust_families: diff_metric_entries(
            load_like_scoped_metric_map(&conn, "family:%", current_build_id)?,
            load_like_scoped_metric_map(&conn, "family:%", previous_build_id)?,
            3,
        ),
    })
}

fn load_build_record(conn: &Connection, build_id: i64) -> Result<Option<BuildRecord>, String> {
    conn.query_row(
        "SELECT b.id, b.created_at, b.elf_path, b.arch, b.rom_bytes, b.ram_bytes, b.warning_count, b.error_count,
                b.metadata_json, b.linker_family, b.map_format,
                g.repo_root, g.commit_hash, g.short_commit_hash, g.branch_name, g.detached_head, g.tag_names_json,
                g.commit_subject, g.author_name, g.author_email, g.commit_timestamp, g.describe, g.is_dirty,
                r.workspace_root, r.manifest_path, r.package_name, r.package_id, r.target_name, r.target_kind_json,
                r.crate_types_json, r.edition, r.target_triple, r.profile, r.artifact_path, r.metadata_source,
                r.workspace_members_json
         FROM builds b
         LEFT JOIN git_metadata g ON g.build_id = b.id
         LEFT JOIN rust_metadata r ON r.build_id = b.id
         WHERE b.id = ?1",
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
                git: parse_git_metadata(row)?,
                rust_context: parse_rust_metadata(row)?,
            })
        },
    )
    .optional()
    .map_err(|err| format!("failed to query build record: {err}"))
}

fn load_metric_map(conn: &Connection, table: &str, name_column: &str, build_id: i64) -> Result<HashMap<String, i64>, String> {
    let sql = format!("SELECT {name_column}, size_bytes FROM {table} WHERE build_id = ?1");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|err| format!("failed to prepare metric query for {table}: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|err| format!("failed to query metric table {table}: {err}"))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect metric table {table}: {err}"))?
        .into_iter()
        .collect())
}

fn load_scoped_metric_map(conn: &Connection, scope: &str, build_id: i64) -> Result<HashMap<String, i64>, String> {
    let mut stmt = conn
        .prepare("SELECT name, size_bytes FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope = ?2")
        .map_err(|err| format!("failed to prepare scoped Rust metric query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id, scope], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|err| format!("failed to query scoped Rust metrics: {err}"))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect scoped Rust metrics: {err}"))?
        .into_iter()
        .collect())
}

fn load_like_scoped_metric_map(conn: &Connection, scope_pattern: &str, build_id: i64) -> Result<HashMap<String, i64>, String> {
    let mut stmt = conn
        .prepare("SELECT name, SUM(size_bytes) FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope LIKE ?2 GROUP BY name")
        .map_err(|err| format!("failed to prepare LIKE-scoped Rust metric query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id, scope_pattern], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|err| format!("failed to query LIKE-scoped Rust metrics: {err}"))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect LIKE-scoped Rust metrics: {err}"))?
        .into_iter()
        .collect())
}

fn diff_metric_entries(current: HashMap<String, i64>, previous: HashMap<String, i64>, limit: usize) -> Vec<ChangeEntry> {
    let mut names = current.keys().cloned().collect::<BTreeSet<_>>();
    names.extend(previous.keys().cloned());
    let mut entries = names
        .into_iter()
        .filter_map(|name| {
            let delta = current.get(&name).copied().unwrap_or(0) - previous.get(&name).copied().unwrap_or(0);
            (delta != 0).then_some(ChangeEntry { name, delta })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| b.delta.abs().cmp(&a.delta.abs()).then_with(|| a.name.cmp(&b.name)));
    entries.truncate(limit);
    entries
}

fn resolve_range_spec(repo: Option<&Path>, spec: &str) -> Result<ResolvedRange, String> {
    if let Some((base, head)) = spec.split_once("...") {
        let resolved_base = resolve_revision(repo, base).ok_or_else(|| format!("failed to resolve revision '{base}'"))?;
        let resolved_head = resolve_revision(repo, head).ok_or_else(|| format!("failed to resolve revision '{head}'"))?;
        let merge = merge_base(repo, base, head).ok_or_else(|| format!("failed to resolve merge-base for '{base}' and '{head}'"))?;
        return Ok(ResolvedRange {
            mode: "triple-dot".to_string(),
            git_range: format!("{merge}..{resolved_head}"),
            resolved_base,
            resolved_head: resolved_head.clone(),
            resolved_merge_base: Some(merge.clone()),
            diff_base: merge,
            diff_head: resolved_head,
        });
    }
    if let Some((base, head)) = spec.split_once("..") {
        let resolved_base = resolve_revision(repo, base).ok_or_else(|| format!("failed to resolve revision '{base}'"))?;
        let resolved_head = resolve_revision(repo, head).ok_or_else(|| format!("failed to resolve revision '{head}'"))?;
        return Ok(ResolvedRange {
            mode: "double-dot".to_string(),
            git_range: format!("{resolved_base}..{resolved_head}"),
            resolved_base: resolved_base.clone(),
            resolved_head: resolved_head.clone(),
            resolved_merge_base: None,
            diff_base: resolved_base,
            diff_head: resolved_head,
        });
    }
    Err(format!("invalid range spec '{spec}', expected A..B or A...B"))
}

fn build_changed_files_summary(
    repo: Option<&Path>,
    base: &str,
    head: &str,
    analysis_changed_files: Vec<String>,
) -> Result<ChangedFilesSummary, String> {
    let git_files = changed_files(repo, base, head)?;
    let git_normalized = git_files
        .iter()
        .map(|item| normalize_repo_path(item))
        .collect::<BTreeSet<_>>();
    let analysis_normalized = analysis_changed_files
        .iter()
        .map(|item| normalize_repo_path(item))
        .collect::<BTreeSet<_>>();
    let intersection_files = git_normalized
        .intersection(&analysis_normalized)
        .cloned()
        .collect::<Vec<_>>();
    Ok(ChangedFilesSummary {
        git_changed_files: git_files,
        changed_source_files_in_analysis: analysis_changed_files,
        git_only_files_count: git_normalized.difference(&analysis_normalized).count(),
        analysis_only_files_count: analysis_normalized.difference(&git_normalized).count(),
        intersection_count: intersection_files.len(),
        intersection_files,
    })
}

fn normalize_repo_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn load_rule_ids_for_build(db_path: &Path, build_id: i64) -> Result<Vec<String>, String> {
    if build_id == 0 {
        return Ok(Vec::new());
    }
    let conn = open_history_db(db_path)?;
    let mut stmt = conn
        .prepare("SELECT code FROM rule_results WHERE build_id = ?1 ORDER BY id ASC")
        .map_err(|err| format!("failed to prepare rule query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| row.get::<_, String>(0))
        .map_err(|err| format!("failed to query rules for build: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect rules for build: {err}"))
}

fn commit_order_name(order: CommitOrder) -> &'static str {
    match order {
        CommitOrder::Timestamp => "timestamp",
        CommitOrder::Ancestry => "ancestry",
    }
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
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

fn query_triple_i64(conn: &Connection, sql: &str, build_id: i64) -> Result<Vec<(String, u64, usize)>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|err| format!("failed to prepare history triple query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, i64>(2)? as usize,
            ))
        })
        .map_err(|err| format!("failed to query history triples: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect history triples: {err}"))
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
        CREATE TABLE IF NOT EXISTS object_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            build_id INTEGER NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
            object_path TEXT NOT NULL,
            size_bytes INTEGER NOT NULL
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
        CREATE TABLE IF NOT EXISTS symbol_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            build_id INTEGER NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            demangled_name TEXT,
            size_bytes INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS why_linked_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            build_id INTEGER NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            confidence TEXT NOT NULL,
            summary TEXT NOT NULL,
            current_size INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS git_metadata (
            build_id INTEGER PRIMARY KEY REFERENCES builds(id) ON DELETE CASCADE,
            repo_root TEXT NOT NULL,
            commit_hash TEXT NOT NULL,
            short_commit_hash TEXT NOT NULL,
            branch_name TEXT,
            detached_head INTEGER NOT NULL,
            tag_names_json TEXT NOT NULL,
            commit_subject TEXT,
            author_name TEXT,
            author_email TEXT,
            commit_timestamp TEXT,
            describe TEXT,
            is_dirty INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS rust_metadata (
            build_id INTEGER PRIMARY KEY REFERENCES builds(id) ON DELETE CASCADE,
            workspace_root TEXT,
            manifest_path TEXT,
            package_name TEXT,
            package_id TEXT,
            target_name TEXT,
            target_kind_json TEXT NOT NULL DEFAULT '[]',
            crate_types_json TEXT NOT NULL DEFAULT '[]',
            edition TEXT,
            target_triple TEXT,
            profile TEXT,
            artifact_path TEXT,
            metadata_source TEXT NOT NULL DEFAULT '',
            workspace_members_json TEXT NOT NULL DEFAULT '[]'
        );
        CREATE TABLE IF NOT EXISTS rust_aggregate_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            build_id INTEGER NOT NULL REFERENCES builds(id) ON DELETE CASCADE,
            scope TEXT NOT NULL,
            name TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            symbol_count INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS schema_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        INSERT INTO schema_meta(key, value) VALUES ('history_schema_version', '6')
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

fn parse_git_metadata(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<GitMetadata>> {
    let repo_root = row.get::<_, Option<String>>(11)?;
    let Some(repo_root) = repo_root else {
        return Ok(None);
    };
    let tag_names = row
        .get::<_, Option<String>>(16)?
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default();
    Ok(Some(GitMetadata {
        repo_root,
        commit_hash: row.get::<_, Option<String>>(12)?.unwrap_or_default(),
        short_commit_hash: row.get::<_, Option<String>>(13)?.unwrap_or_default(),
        branch_name: row.get(14)?,
        detached_head: row.get::<_, Option<i64>>(15)?.unwrap_or(0) != 0,
        tag_names,
        commit_subject: row.get(17)?,
        author_name: row.get(18)?,
        author_email: row.get(19)?,
        commit_timestamp: row.get(20)?,
        describe: row.get(21)?,
        is_dirty: row.get::<_, Option<i64>>(22)?.unwrap_or(0) != 0,
    }))
}

fn parse_rust_metadata(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<RustContext>> {
    let workspace_root = row.get::<_, Option<String>>(23)?;
    let manifest_path = row.get::<_, Option<String>>(24)?;
    let package_name = row.get::<_, Option<String>>(25)?;
    let package_id = row.get::<_, Option<String>>(26)?;
    let target_name = row.get::<_, Option<String>>(27)?;
    let metadata_source = row.get::<_, Option<String>>(34)?;
    if workspace_root.is_none()
        && manifest_path.is_none()
        && package_name.is_none()
        && package_id.is_none()
        && target_name.is_none()
        && metadata_source.as_deref().unwrap_or_default().is_empty()
    {
        return Ok(None);
    }
    let target_kind = row
        .get::<_, Option<String>>(28)?
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default();
    let crate_types = row
        .get::<_, Option<String>>(29)?
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default();
    let workspace_members = row
        .get::<_, Option<String>>(35)?
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default();
    Ok(Some(RustContext {
        workspace_root,
        manifest_path,
        package_name,
        package_id,
        target_name,
        target_kind,
        crate_types,
        edition: row.get(30)?,
        target_triple: row.get(31)?,
        profile: row.get(32)?,
        artifact_path: row.get(33)?,
        metadata_source: metadata_source.unwrap_or_default(),
        workspace_members,
    }))
}

fn format_git_summary(git: Option<&GitMetadata>) -> String {
    let Some(git) = git else {
        return String::new();
    };
    let mut parts = vec![git.short_commit_hash.clone()];
    if let Some(branch) = git.branch_name.as_deref() {
        parts.push(branch.to_string());
    } else if git.detached_head {
        parts.push("detached".to_string());
    }
    if let Some(describe) = git.describe.as_deref() {
        parts.push(describe.to_string());
    }
    if git.is_dirty {
        parts.push("dirty".to_string());
    }
    format!(" | git={}", parts.join(" / "))
}

fn collect_why_linked_records(analysis: &AnalysisResult, limit: usize) -> Vec<WhyLinkedRecord> {
    let mut totals = BTreeMap::<String, u64>::new();
    for item in &analysis.object_contributions {
        *totals.entry(item.object_path.clone()).or_default() += item.size;
    }
    let mut entries = totals.into_iter().collect::<Vec<_>>();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    entries
        .into_iter()
        .take(limit)
        .filter_map(|(target, current_size)| {
            explain_object(analysis, &target).map(|item| WhyLinkedRecord {
                target,
                kind: "object".to_string(),
                confidence: item.confidence.to_string(),
                summary: item.summary,
                current_size,
            })
        })
        .collect()
}

fn query_why_linked_trend(conn: &Connection, target: &str, kind: &str, last: usize) -> Result<Vec<TrendPoint>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.created_at, w.current_size, w.confidence, w.summary
             FROM builds b
             JOIN why_linked_metrics w ON w.build_id = b.id
             WHERE w.target = ?1 AND w.kind = ?2
             ORDER BY b.id DESC LIMIT ?3",
        )
        .map_err(|err| format!("failed to prepare why-linked trend query: {err}"))?;
    let rows = stmt
        .query_map(params![target, kind, last as i64], |row| {
            let confidence: String = row.get(3)?;
            let summary: String = row.get(4)?;
            Ok(TrendPoint {
                build_id: row.get(0)?,
                created_at: row.get(1)?,
                label: target.to_string(),
                value: row.get::<_, i64>(2)?,
                format: TrendFormat::Bytes,
                note: Some(format!("[{}] {}", confidence, summary)),
            })
        })
        .map_err(|err| format!("failed to query why-linked trend metric: {err}"))?;
    let mut points = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to collect why-linked trend metric: {err}"))?;
    points.reverse();
    Ok(points)
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::{
        commit_timeline, list_builds, range_diff, record_build, regression_origin, show_build, trend_metric,
        variant_key, BuildRecord, HistoryRecordInput, RegressionConfidence, RegressionDetector, RegressionMode,
        TrendFormat,
    };
    use crate::git::{collect_git_metadata, CommitOrder, GitOptions};
    use crate::model::{
        AnalysisResult, BinaryInfo, DebugArtifactInfo, DebugInfoSummary, MemorySummary, ObjectContribution,
        ObjectSourceKind, RustAggregate, RustContext, RustFamilyKind, RustFamilySummary, RustSymbolSummary, RustView,
        SectionCategory, SectionTotal, SymbolInfo, SymbolLanguage, ToolchainInfo, ToolchainKind, ToolchainSelection,
        UnknownSourceBucket, WarningItem, WarningLevel, WarningSource,
    };
    use rusqlite::Connection;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use std::process::Command;
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
        assert_eq!(detail.why_linked.len(), 0);
        let _ = fs::remove_file(db);
    }

    #[test]
    fn persists_rust_view_aggregates_and_supports_rust_trend_and_regression() {
        let db = temp_db();
        let repo = init_repo("rust-metrics");
        let commit_one = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit_with_rust(100, 20, &commit_one, "tokio", 32),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();
        fs::write(repo.join("src").join("main.cpp"), "hello 2\n").unwrap();
        run_git(&repo, &["commit", "-am", "second commit"]);
        let commit_two = checkout_and_collect(&repo, "HEAD");
        let build_id = record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit_with_rust(140, 24, &commit_two, "tokio", 64),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();
        fs::write(repo.join("src").join("main.cpp"), "hello 3\n").unwrap();
        run_git(&repo, &["commit", "-am", "third commit"]);
        let commit_three = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit_with_rust(150, 28, &commit_three, "tokio", 72),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();

        let detail = show_build(&db, build_id).unwrap().unwrap();
        assert_eq!(detail.rust_packages.first().map(|item| item.0.as_str()), Some("fwmap"));
        assert_eq!(detail.rust_targets.first().map(|item| item.0.as_str()), Some("fwmap"));
        assert_eq!(detail.rust_crates.first().map(|item| item.0.as_str()), Some("fwmap"));
        assert_eq!(detail.rust_dependencies.first().map(|item| item.0.as_str()), Some("tokio"));
        assert_eq!(detail.rust_source_files.first().map(|item| item.0.as_str()), Some("src/main.rs"));

        let trend = trend_metric(&db, "rust-dependency:tokio", 10).unwrap();
        assert_eq!(trend.len(), 3);
        assert_eq!(trend[0].value, 32);
        assert_eq!(trend[1].value, 64);
        assert_eq!(trend[2].value, 72);

        let regression = regression_origin(
            &db,
            Some(&repo),
            "HEAD~2..HEAD",
            RegressionDetector::Metric,
            "rust-dependency:tokio.size",
            RegressionMode::FirstCrossing,
            Some(36),
            None,
            None,
            CommitOrder::Ancestry,
            false,
            false,
            false,
            8,
            None,
            Some("release"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            regression.origin.first_observed_bad.as_ref().map(|item| item.subject.as_str()),
            Some("third commit")
        );

        let _ = fs::remove_file(db);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn persists_and_loads_rust_context() {
        let db = temp_db();
        let mut analysis = sample_analysis(1024, 256, 0);
        analysis.rust_context = Some(RustContext {
            workspace_root: Some("/workspace/fwmap".to_string()),
            manifest_path: Some("/workspace/fwmap/Cargo.toml".to_string()),
            package_name: Some("fwmap".to_string()),
            package_id: Some("path+file:///workspace/fwmap#fwmap@0.1.0".to_string()),
            target_name: Some("fwmap".to_string()),
            target_kind: vec!["bin".to_string()],
            crate_types: vec!["bin".to_string()],
            edition: Some("2024".to_string()),
            target_triple: Some("x86_64-unknown-linux-gnu".to_string()),
            profile: Some("release".to_string()),
            artifact_path: Some("/workspace/fwmap/target/release/fwmap".to_string()),
            metadata_source: "cargo-build-json".to_string(),
            workspace_members: vec!["fwmap".to_string()],
        });
        let id = record_build(
            &db,
            HistoryRecordInput {
                analysis,
                metadata: BTreeMap::new(),
            },
        )
        .unwrap();
        let items = list_builds(&db).unwrap();
        assert_eq!(items[0].rust_context.as_ref().and_then(|item| item.package_name.as_deref()), Some("fwmap"));
        let detail = show_build(&db, id).unwrap().unwrap();
        assert_eq!(
            detail
                .build
                .rust_context
                .as_ref()
                .and_then(|item| item.target_triple.as_deref()),
            Some("x86_64-unknown-linux-gnu")
        );
        let _ = fs::remove_file(db);
    }

    #[test]
    fn migrates_old_history_db_without_rust_metadata_table() {
        let db = temp_db();
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE builds (
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
            CREATE TABLE git_metadata (
                build_id INTEGER PRIMARY KEY,
                repo_root TEXT NOT NULL,
                commit_hash TEXT NOT NULL,
                short_commit_hash TEXT NOT NULL,
                branch_name TEXT,
                detached_head INTEGER NOT NULL,
                tag_names_json TEXT NOT NULL,
                commit_subject TEXT,
                author_name TEXT,
                author_email TEXT,
                commit_timestamp TEXT,
                describe TEXT,
                is_dirty INTEGER NOT NULL
            );
            CREATE TABLE schema_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO builds (
                created_at, elf_path, arch, linker_family, map_format, rom_bytes, ram_bytes, warning_count, error_count, metadata_json
            ) VALUES (1, 'legacy.elf', 'ARM', 'gnu', 'unknown', 10, 2, 0, 0, '{}');
            INSERT INTO schema_meta(key, value) VALUES ('history_schema_version', '5');
            ",
        )
        .unwrap();
        drop(conn);

        let items = list_builds(&db).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].rust_context.is_none());
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

    #[test]
    fn stores_why_linked_records_and_exposes_object_trend_notes() {
        let db = temp_db();
        let mut analysis = sample_analysis(100, 10, 0);
        analysis.object_contributions = vec![ObjectContribution {
            object_path: "build/main.o".to_string(),
            source_kind: ObjectSourceKind::Object,
            section_name: Some(".text".to_string()),
            size: 100,
        }];
        record_build(
            &db,
            HistoryRecordInput {
                analysis,
                metadata: BTreeMap::new(),
            },
        )
        .unwrap();
        let detail = show_build(&db, 1).unwrap().unwrap();
        assert_eq!(detail.why_linked.len(), 1);
        let trend = trend_metric(&db, "object:build/main.o", 10).unwrap();
        assert_eq!(trend[0].value, 100);
        assert!(trend[0].note.as_deref().unwrap_or_default().contains("linked"));
        let _ = fs::remove_file(db);
    }

    #[test]
    fn builds_commit_timeline_with_deltas() {
        let db = temp_db();
        let repo = init_repo("timeline");
        let commit_one = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit(100, 20, &commit_one),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();
        fs::write(repo.join("src").join("main.cpp"), "hello 2\n").unwrap();
        run_git(&repo, &["commit", "-am", "second commit"]);
        let commit_two = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit(140, 24, &commit_two),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();

        let report = commit_timeline(&db, Some(&repo), None, 10, Some("release"), None, None, CommitOrder::Ancestry).unwrap();
        assert_eq!(report.rows.len(), 2);
        assert_eq!(report.rows[0].subject, "second commit");
        assert_eq!(report.rows[1].subject, "initial commit");
        assert_eq!(report.rows[0].rom_delta_vs_previous, Some(40));
        assert_eq!(report.rows[1].rom_delta_vs_previous, None);
        let _ = fs::remove_file(db);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn builds_range_diff_and_reports_missing_analysis() {
        let db = temp_db();
        let repo = init_repo("range");
        let _commit_one = checkout_and_collect(&repo, "HEAD");
        fs::write(repo.join("src").join("main.cpp"), "hello 2\n").unwrap();
        run_git(&repo, &["commit", "-am", "second commit"]);
        let commit_two = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit(120, 22, &commit_two),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();
        fs::write(repo.join("src").join("main.cpp"), "hello 3\n").unwrap();
        run_git(&repo, &["commit", "-am", "third commit"]);
        let commit_three = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit(180, 30, &commit_three),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();

        let report = range_diff(&db, Some(&repo), "HEAD~2..HEAD", CommitOrder::Timestamp, true, Some("release"), None, None)
            .unwrap();
        assert_eq!(report.total_commits_in_git_range, 2);
        assert_eq!(report.analyzed_commits_count, 2);
        assert_eq!(report.missing_analysis_commits_count, 0);
        assert_eq!(report.timeline_rows[0].subject, "third commit");
        assert_eq!(report.timeline_rows[0].rom_delta_vs_previous, Some(60));
        assert_eq!(report.timeline_rows[1].rom_delta_vs_previous, None);
        assert_eq!(report.cumulative_rom_delta, 60);
        assert!(report.changed_files_summary.as_ref().unwrap().intersection_count >= 1);
        let _ = fs::remove_file(db);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn detects_metric_regression_origin() {
        let db = temp_db();
        let repo = init_repo("regression-metric");
        let commit_one = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit(100, 20, &commit_one),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();
        fs::write(repo.join("src").join("main.cpp"), "hello 2\n").unwrap();
        run_git(&repo, &["commit", "-am", "second commit"]);
        let commit_two = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit(112, 21, &commit_two),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();
        fs::write(repo.join("src").join("main.cpp"), "hello 3\n").unwrap();
        run_git(&repo, &["commit", "-am", "third commit"]);
        let commit_three = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit(140, 24, &commit_three),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();

        let report = regression_origin(
            &db,
            Some(&repo),
            "HEAD~2..HEAD",
            RegressionDetector::Metric,
            "rom_total",
            RegressionMode::FirstCrossing,
            Some(30),
            None,
            None,
            CommitOrder::Ancestry,
            true,
            true,
            false,
            8,
            None,
            Some("release"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(report.summary.confidence, RegressionConfidence::High);
        assert_eq!(report.origin.last_good.as_ref().map(|item| item.subject.as_str()), Some("second commit"));
        assert_eq!(
            report.origin.first_observed_bad.as_ref().map(|item| item.subject.as_str()),
            Some("third commit")
        );
        assert!(report.evidence.as_ref().unwrap().changed_files.as_ref().unwrap().intersection_count >= 1);
        let _ = fs::remove_file(db);
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn detects_rule_and_entity_regression_origin() {
        let db = temp_db();
        let repo = init_repo("regression-rule");
        let commit_one = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit(100, 20, &commit_one),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();
        fs::write(repo.join("src").join("main.cpp"), "hello 2\n").unwrap();
        run_git(&repo, &["commit", "-am", "second commit"]);
        let commit_two = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit_with_warning(120, 22, &commit_two, "ram-budget-exceeded"),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();
        fs::write(repo.join("src").join("feature.cpp"), "feature\n").unwrap();
        run_git(&repo, &["add", "src/feature.cpp"]);
        run_git(&repo, &["commit", "-m", "third commit"]);
        let commit_three = checkout_and_collect(&repo, "HEAD");
        record_build(
            &db,
            HistoryRecordInput {
                analysis: analysis_for_commit_with_source(150, 30, &commit_three, "src/feature.cpp"),
                metadata: metadata_for_profile("release"),
            },
        )
        .unwrap();

        let rule_report = regression_origin(
            &db,
            Some(&repo),
            "HEAD~2..HEAD",
            RegressionDetector::Rule,
            "ram-budget-exceeded",
            RegressionMode::FirstViolation,
            None,
            None,
            None,
            CommitOrder::Ancestry,
            true,
            false,
            false,
            8,
            None,
            Some("release"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            rule_report.origin.first_observed_bad.as_ref().map(|item| item.subject.as_str()),
            Some("second commit")
        );

        let entity_report = regression_origin(
            &db,
            Some(&repo),
            "HEAD~2..HEAD",
            RegressionDetector::Entity,
            "source:src/feature.cpp",
            RegressionMode::FirstPresence,
            None,
            None,
            None,
            CommitOrder::Ancestry,
            false,
            false,
            false,
            8,
            None,
            Some("release"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            entity_report.origin.first_observed_bad.as_ref().map(|item| item.subject.as_str()),
            Some("third commit")
        );
        let _ = fs::remove_file(db);
        let _ = fs::remove_dir_all(repo);
    }

    fn temp_db() -> std::path::PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("fwmap-history-{nanos}.db"))
    }

    fn metadata_for_profile(profile: &str) -> BTreeMap<String, String> {
        let mut metadata = BTreeMap::new();
        metadata.insert("build.profile".to_string(), profile.to_string());
        metadata.insert("toolchain.id".to_string(), "gnu".to_string());
        metadata.insert("config.fingerprint".to_string(), "gnu|unknown|all".to_string());
        metadata
    }

    #[test]
    fn variant_key_ignores_source_lines_component_in_config_fingerprint() {
        let mut build_off = BuildRecord {
            id: 1,
            created_at: 0,
            elf_path: "a.elf".to_string(),
            arch: "ARM".to_string(),
            linker_family: "gnu".to_string(),
            map_format: "unknown".to_string(),
            rom_bytes: 1,
            ram_bytes: 1,
            warning_count: 0,
            error_count: 0,
            metadata: BTreeMap::new(),
            git: None,
            rust_context: None,
        };
        build_off.metadata.insert("build.profile".to_string(), "release".to_string());
        build_off.metadata.insert("toolchain.id".to_string(), "gnu".to_string());
        build_off
            .metadata
            .insert("config.fingerprint".to_string(), "gnu|unknown|off".to_string());

        let mut build_lines = build_off.clone();
        build_lines
            .metadata
            .insert("config.fingerprint".to_string(), "gnu|unknown|lines".to_string());

        assert_eq!(variant_key(&build_off), variant_key(&build_lines));
    }

    fn analysis_for_commit(rom: u64, ram: u64, git: &crate::model::GitMetadata) -> AnalysisResult {
        let mut analysis = sample_analysis(rom, ram, 0);
        analysis.git = Some(git.clone());
        analysis.object_contributions = vec![ObjectContribution {
            object_path: "build/main.o".to_string(),
            source_kind: ObjectSourceKind::Object,
            section_name: Some(".text".to_string()),
            size: rom,
        }];
        analysis.symbols = vec![SymbolInfo {
            name: "main".to_string(),
            demangled_name: Some("main()".to_string()),
            section_name: Some(".text".to_string()),
            object_path: Some("build/main.o".to_string()),
            addr: 0,
            size: rom,
        }];
        analysis
    }

    fn analysis_for_commit_with_warning(
        rom: u64,
        ram: u64,
        git: &crate::model::GitMetadata,
        code: &str,
    ) -> AnalysisResult {
        let mut analysis = analysis_for_commit(rom, ram, git);
        analysis.warnings.push(WarningItem {
            level: WarningLevel::Warn,
            code: code.to_string(),
            message: "warning".to_string(),
            source: WarningSource::Analyze,
            related: None,
        });
        analysis
    }

    fn analysis_for_commit_with_source(
        rom: u64,
        ram: u64,
        git: &crate::model::GitMetadata,
        source_path: &str,
    ) -> AnalysisResult {
        let mut analysis = analysis_for_commit(rom, ram, git);
        analysis.source_files.push(crate::model::SourceFile {
            path: source_path.to_string(),
            display_path: source_path.to_string(),
            directory: Path::new(source_path)
                .parent()
                .and_then(|item| item.to_str())
                .unwrap_or_default()
                .to_string(),
            size: rom / 2,
            functions: 1,
            line_ranges: 1,
        });
        analysis
    }

    fn analysis_for_commit_with_rust(
        rom: u64,
        ram: u64,
        git: &crate::model::GitMetadata,
        dependency: &str,
        dependency_size: u64,
    ) -> AnalysisResult {
        let mut analysis = analysis_for_commit(rom, ram, git);
        analysis.rust_context = Some(RustContext {
            workspace_root: Some("/workspace/fwmap".to_string()),
            manifest_path: Some("/workspace/fwmap/Cargo.toml".to_string()),
            package_name: Some("fwmap".to_string()),
            package_id: Some("path+file:///workspace/fwmap#fwmap@0.1.0".to_string()),
            target_name: Some("fwmap".to_string()),
            target_kind: vec!["bin".to_string()],
            crate_types: vec!["bin".to_string()],
            edition: Some("2024".to_string()),
            target_triple: Some("x86_64-unknown-linux-gnu".to_string()),
            profile: Some("release".to_string()),
            artifact_path: Some("/workspace/fwmap/target/release/fwmap".to_string()),
            metadata_source: "test".to_string(),
            workspace_members: vec!["fwmap".to_string()],
        });
        analysis.rust_view = Some(RustView {
            workspace: Some("/workspace/fwmap".to_string()),
            packages: vec![RustAggregate {
                name: "fwmap".to_string(),
                size: rom,
                symbol_count: 2,
            }],
            targets: vec![RustAggregate {
                name: "fwmap".to_string(),
                size: rom,
                symbol_count: 2,
            }],
            crates: vec![
                RustAggregate {
                    name: "fwmap".to_string(),
                    size: rom.saturating_sub(dependency_size),
                    symbol_count: 1,
                },
                RustAggregate {
                    name: dependency.to_string(),
                    size: dependency_size,
                    symbol_count: 1,
                },
            ],
            dependency_crates: vec![RustAggregate {
                name: dependency.to_string(),
                size: dependency_size,
                symbol_count: 1,
            }],
            source_files: vec![RustAggregate {
                name: "src/main.rs".to_string(),
                size: rom,
                symbol_count: 2,
            }],
            grouped_families: vec![RustFamilySummary {
                kind: RustFamilyKind::Async,
                key: "fwmap::worker::poll".to_string(),
                display_name: "fwmap::worker::poll".to_string(),
                size: rom,
                symbol_count: 2,
            }],
            symbols: vec![RustSymbolSummary {
                raw_name: "_RNvC6fwmap6worker4poll".to_string(),
                demangled_name: Some("fwmap::worker::poll".to_string()),
                display_name: "fwmap::worker::poll".to_string(),
                language: SymbolLanguage::Rust,
                package: Some("fwmap".to_string()),
                target: Some("fwmap".to_string()),
                crate_name: Some("fwmap".to_string()),
                dependency_crate: Some(dependency.to_string()),
                source_path: Some("src/main.rs".to_string()),
                family_kind: RustFamilyKind::Async,
                family_key: "fwmap::worker::poll".to_string(),
                size: rom,
            }],
            total_rust_size: rom,
        });
        analysis
    }

    fn init_repo(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("fwmap-history-{label}-{}", super::now_unix()));
        fs::create_dir_all(&dir).unwrap();
        run_git(&dir, &["init"]);
        run_git(&dir, &["config", "user.name", "fwmap test"]);
        run_git(&dir, &["config", "user.email", "fwmap@example.com"]);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src").join("main.cpp"), "hello\n").unwrap();
        run_git(&dir, &["add", "src/main.cpp"]);
        run_git(&dir, &["commit", "-m", "initial commit"]);
        run_git(&dir, &["branch", "-M", "main"]);
        dir
    }

    fn checkout_and_collect(repo: &Path, revision: &str) -> crate::model::GitMetadata {
        run_git(repo, &["checkout", revision]);
        collect_git_metadata(&GitOptions {
            enabled: true,
            repo_path: Some(repo.to_path_buf()),
        })
        .unwrap()
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap();
        assert!(status.success(), "git {:?} failed", args);
    }

    fn sample_analysis(rom: u64, ram: u64, warnings: usize) -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "sample.elf".to_string(),
                arch: "ARM".to_string(),
                elf_class: "ELF32".to_string(),
                endian: "little-endian".to_string(),
            },
            git: None,
            rust_context: None,
            rust_view: None,
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
            policy: None,
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
            whole_archive_candidates: Vec::new(),
            relocation_references: Vec::new(),
            cross_references: Vec::new(),
            cpp_view: crate::model::CppView::default(),
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
