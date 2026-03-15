use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopAppInfo {
    pub app_name: String,
    pub app_version: String,
    pub cli_version: String,
    pub history_db_path: String,
    pub app_db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRequestDto {
    pub elf_path: Option<String>,
    pub map_path: Option<String>,
    pub debug_path: Option<String>,
    pub rule_file_path: Option<String>,
    pub git_repo_path: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettingsDto {
    pub history_db_path: String,
    pub default_rule_file_path: Option<String>,
    pub default_git_repo_path: Option<String>,
    pub last_elf_path: Option<String>,
    pub last_map_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatusDto {
    pub job_id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub label: Option<String>,
    pub progress_message: String,
    pub error_message: Option<String>,
    pub run_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobEventDto {
    pub job_id: String,
    pub status: String,
    pub message: String,
    pub run_id: Option<i64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummaryDto {
    pub run_id: i64,
    pub build_id: i64,
    pub created_at: String,
    pub label: Option<String>,
    pub status: String,
    pub git_revision: Option<String>,
    pub profile: Option<String>,
    pub target: Option<String>,
    pub rom_bytes: u64,
    pub ram_bytes: u64,
    pub warning_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetailDto {
    pub run: RunSummaryDto,
    pub elf_path: String,
    pub arch: String,
    pub linker_family: String,
    pub map_format: String,
    pub report_html_path: Option<String>,
    pub report_json_path: Option<String>,
    pub git_branch: Option<String>,
    pub git_describe: Option<String>,
    pub top_sections: Vec<(String, u64)>,
    pub top_symbols: Vec<(String, u64)>,
    pub warnings: Vec<(String, String, Option<String>)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQueryDto {
    pub repo_path: Option<String>,
    pub branch: Option<String>,
    pub profile: Option<String>,
    pub toolchain: Option<String>,
    pub target: Option<String>,
    pub limit: Option<usize>,
    pub order: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItemDto {
    pub build_id: i64,
    pub created_at: String,
    pub elf_path: String,
    pub arch: String,
    pub linker_family: String,
    pub map_format: String,
    pub rom_bytes: u64,
    pub ram_bytes: u64,
    pub warning_count: u64,
    pub error_count: u64,
    pub git_revision: Option<String>,
    pub git_branch: Option<String>,
    pub git_subject: Option<String>,
    pub git_describe: Option<String>,
    pub profile: Option<String>,
    pub target: Option<String>,
    pub toolchain_id: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeltaEntryDto {
    pub name: String,
    pub delta: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntryDto {
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
    pub rom_total: u64,
    pub ram_total: u64,
    pub rom_delta_vs_previous: Option<i64>,
    pub ram_delta_vs_previous: Option<i64>,
    pub rule_violations_count: u64,
    pub top_sections: Vec<DeltaEntryDto>,
    pub top_objects: Vec<DeltaEntryDto>,
    pub top_source_files: Vec<DeltaEntryDto>,
    pub top_symbols: Vec<DeltaEntryDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineResultDto {
    pub repo_id: String,
    pub order: String,
    pub branch: Option<String>,
    pub profile: Option<String>,
    pub toolchain: Option<String>,
    pub target: Option<String>,
    pub rows: Vec<TimelineEntryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCompareRequestDto {
    pub left_run_id: i64,
    pub right_run_id: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricSummaryDto {
    pub rom_delta: i64,
    pub ram_delta: i64,
    pub warning_delta: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCompareResultDto {
    pub left_run: RunSummaryDto,
    pub right_run: RunSummaryDto,
    pub summary: MetricSummaryDto,
    pub region_deltas: Vec<DeltaEntryDto>,
    pub section_deltas: Vec<DeltaEntryDto>,
    pub object_deltas: Vec<DeltaEntryDto>,
    pub source_file_deltas: Vec<DeltaEntryDto>,
    pub symbol_deltas: Vec<DeltaEntryDto>,
    pub rust_dependency_deltas: Vec<DeltaEntryDto>,
    pub rust_family_deltas: Vec<DeltaEntryDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRefDto {
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFilesSummaryDto {
    pub git_changed_files: Vec<String>,
    pub changed_source_files_in_analysis: Vec<String>,
    pub intersection_files: Vec<String>,
    pub git_only_files_count: usize,
    pub analysis_only_files_count: usize,
    pub intersection_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorstCommitSummaryDto {
    pub commit: String,
    pub delta: i64,
    pub subject: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstRuleViolationSummaryDto {
    pub commit: String,
    pub rule_ids: Vec<String>,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeDiffQueryDto {
    pub repo_path: Option<String>,
    pub spec: String,
    pub include_changed_files: Option<bool>,
    pub order: Option<String>,
    pub profile: Option<String>,
    pub toolchain: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeDiffResultDto {
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
    pub cumulative_rom_delta: i64,
    pub cumulative_ram_delta: i64,
    pub worst_commit_by_rom: Option<WorstCommitSummaryDto>,
    pub worst_commit_by_ram: Option<WorstCommitSummaryDto>,
    pub first_rule_violation: Option<FirstRuleViolationSummaryDto>,
    pub top_changed_sections: Vec<DeltaEntryDto>,
    pub top_changed_objects: Vec<DeltaEntryDto>,
    pub top_changed_source_files: Vec<DeltaEntryDto>,
    pub top_changed_symbols: Vec<DeltaEntryDto>,
    pub top_changed_rust_dependencies: Vec<DeltaEntryDto>,
    pub top_changed_rust_families: Vec<DeltaEntryDto>,
    pub changed_files_summary: Option<ChangedFilesSummaryDto>,
    pub timeline_rows: Vec<TimelineEntryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionQueryDto {
    pub repo_path: Option<String>,
    pub spec: String,
    pub detector_type: String,
    pub key: String,
    pub mode: String,
    pub threshold: Option<i64>,
    pub threshold_percent: Option<f64>,
    pub jump_threshold: Option<i64>,
    pub order: Option<String>,
    pub include_evidence: Option<bool>,
    pub include_changed_files: Option<bool>,
    pub bisect_like: Option<bool>,
    pub max_steps: Option<usize>,
    pub limit_commits: Option<usize>,
    pub profile: Option<String>,
    pub toolchain: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionOriginPointDto {
    pub commit: String,
    pub short_commit: String,
    pub subject: String,
    pub value: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionWindowRowDto {
    pub commit: String,
    pub short_commit: String,
    pub subject: String,
    pub status: String,
    pub value: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionResultDto {
    pub repo_id: String,
    pub detector_type: String,
    pub key: String,
    pub mode: String,
    pub confidence: String,
    pub reasoning: String,
    pub searched_commit_count: usize,
    pub analyzed_commit_count: usize,
    pub missing_analysis_count: usize,
    pub mixed_configuration: bool,
    pub last_good: Option<RegressionOriginPointDto>,
    pub first_observed_bad: Option<RegressionOriginPointDto>,
    pub first_bad_candidate: Option<RegressionOriginPointDto>,
    pub transition_window: Vec<RegressionWindowRowDto>,
    pub top_growth_sections: Vec<DeltaEntryDto>,
    pub top_growth_objects: Vec<DeltaEntryDto>,
    pub top_growth_source_files: Vec<DeltaEntryDto>,
    pub top_growth_symbols: Vec<DeltaEntryDto>,
    pub changed_files_summary: Option<ChangedFilesSummaryDto>,
    pub related_rule_hits: Vec<String>,
    pub narrowed_commits: Vec<String>,
}
