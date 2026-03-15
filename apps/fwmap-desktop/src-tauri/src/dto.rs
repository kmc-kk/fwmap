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


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DashboardQueryDto {
    pub repo_path: Option<String>,
    pub branch: Option<String>,
    pub profile: Option<String>,
    pub toolchain: Option<String>,
    pub target: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewCardDto {
    pub key: String,
    pub title: String,
    pub value: String,
    pub subtitle: Option<String>,
    pub tone: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendPointDto {
    pub label: String,
    pub value: f64,
    pub secondary_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendSeriesDto {
    pub key: String,
    pub label: String,
    pub unit: String,
    pub points: Vec<TrendPointDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopGrowthEntryDto {
    pub scope: String,
    pub name: String,
    pub delta: i64,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentRegressionDto {
    pub detector_type: String,
    pub key: String,
    pub confidence: String,
    pub commit: String,
    pub subject: String,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionUsageDto {
    pub region_name: String,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub usage_ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSummaryDto {
    pub overview_cards: Vec<OverviewCardDto>,
    pub latest_run: Option<RunSummaryDto>,
    pub latest_history_item: Option<HistoryItemDto>,
    pub recent_trends: Vec<TrendSeriesDto>,
    pub recent_regressions: Vec<RecentRegressionDto>,
    pub top_growth_sources: Vec<TopGrowthEntryDto>,
    pub region_usage: Vec<RegionUsageDto>,
}


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummaryDto {
    pub project_id: i64,
    pub name: String,
    pub root_path: String,
    pub git_repo_path: Option<String>,
    pub default_rule_file_path: Option<String>,
    pub default_target: Option<String>,
    pub default_profile: Option<String>,
    pub last_run_at: Option<String>,
    pub last_export_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetailDto {
    pub project_id: i64,
    pub name: String,
    pub root_path: String,
    pub git_repo_path: Option<String>,
    pub default_elf_path: Option<String>,
    pub default_map_path: Option<String>,
    pub default_debug_path: Option<String>,
    pub default_rule_file_path: Option<String>,
    pub default_target: Option<String>,
    pub default_profile: Option<String>,
    pub default_export_dir: Option<String>,
    pub pinned_report_path: Option<String>,
    pub last_opened_screen: Option<String>,
    pub last_opened_filters_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequestDto {
    pub name: String,
    pub root_path: String,
    pub git_repo_path: Option<String>,
    pub default_elf_path: Option<String>,
    pub default_map_path: Option<String>,
    pub default_debug_path: Option<String>,
    pub default_rule_file_path: Option<String>,
    pub default_target: Option<String>,
    pub default_profile: Option<String>,
    pub default_export_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectRequestDto {
    pub name: Option<String>,
    pub root_path: Option<String>,
    pub git_repo_path: Option<String>,
    pub default_elf_path: Option<String>,
    pub default_map_path: Option<String>,
    pub default_debug_path: Option<String>,
    pub default_rule_file_path: Option<String>,
    pub default_target: Option<String>,
    pub default_profile: Option<String>,
    pub default_export_dir: Option<String>,
    pub pinned_report_path: Option<String>,
    pub last_opened_screen: Option<String>,
    pub last_opened_filters_json: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveProjectStateDto {
    pub active_project_id: Option<i64>,
    pub active_project: Option<ProjectDetailDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDocumentDto {
    pub path: Option<String>,
    pub format: String,
    pub content: String,
    pub project_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyValidationIssueDto {
    pub level: String,
    pub message: String,
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyValidationResultDto {
    pub ok: bool,
    pub issues: Vec<PolicyValidationIssueDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentExportDto {
    pub export_id: i64,
    pub project_id: Option<i64>,
    pub created_at: String,
    pub export_target: String,
    pub format: String,
    pub destination_path: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequestDto {
    pub project_id: Option<i64>,
    pub export_target: String,
    pub format: String,
    pub destination_path: String,
    pub run_id: Option<i64>,
    pub compare: Option<RunCompareRequestDto>,
    pub history_query: Option<HistoryQueryDto>,
    pub range_query: Option<RangeDiffQueryDto>,
    pub regression_query: Option<RegressionQueryDto>,
    pub dashboard_query: Option<DashboardQueryDto>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResultDto {
    pub destination_path: String,
    pub export_target: String,
    pub format: String,
    pub created_at: String,
}


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InspectorQueryDto {
    pub run_id: Option<i64>,
    pub build_id: Option<i64>,
    pub left_run_id: Option<i64>,
    pub right_run_id: Option<i64>,
    pub view_mode: String,
    pub group_by: String,
    pub metric: String,
    pub search: Option<String>,
    pub top_n: Option<usize>,
    pub threshold_min: Option<i64>,
    pub only_increased: Option<bool>,
    pub only_decreased: Option<bool>,
    pub debug_info_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSelectionDto {
    pub stable_id: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSummaryDto {
    pub context_label: String,
    pub source_kind: String,
    pub entity_count: usize,
    pub total_size_bytes: u64,
    pub total_delta_bytes: i64,
    pub debug_info_available: bool,
    pub available_views: Vec<String>,
    pub available_visualizations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorItemDto {
    pub stable_id: String,
    pub display_label: String,
    pub raw_label: String,
    pub kind: String,
    pub size_bytes: u64,
    pub delta_bytes: i64,
    pub percentage: f64,
    pub parent_id: Option<String>,
    pub has_children: bool,
    pub source_available: bool,
    pub metadata: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorBreakdownDto {
    pub query: InspectorQueryDto,
    pub items: Vec<InspectorItemDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorHierarchyNodeDto {
    pub stable_id: String,
    pub label: String,
    pub kind: String,
    pub size_bytes: u64,
    pub delta_bytes: i64,
    pub percentage: f64,
    pub source_available: bool,
    pub children: Vec<InspectorHierarchyNodeDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectorDetailDto {
    pub stable_id: String,
    pub label: String,
    pub kind: String,
    pub size_bytes: u64,
    pub delta_bytes: i64,
    pub parent_label: Option<String>,
    pub source_available: bool,
    pub metadata: std::collections::BTreeMap<String, String>,
    pub related_rule_violations: Vec<String>,
    pub related_regression_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceContextDto {
    pub path: Option<String>,
    pub function_name: Option<String>,
    pub line_start: Option<u64>,
    pub line_end: Option<u64>,
    pub excerpt: Option<String>,
    pub compile_unit: Option<String>,
    pub crate_name: Option<String>,
    pub related_sections: Vec<String>,
    pub related_regions: Vec<String>,
    pub availability_reason: Option<String>,
}
