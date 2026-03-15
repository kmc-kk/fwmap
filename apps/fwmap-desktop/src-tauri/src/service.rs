use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use chrono::{TimeZone, Utc};
use fwmap::core::analyze::{AnalyzeOptions, analyze_paths};
use fwmap::core::git::{CommitOrder, GitOptions};
use fwmap::core::history::{
    HistoryRecordInput, RegressionConfidence, RegressionDetector, RegressionMode, commit_timeline,
    list_builds, range_diff, record_build, regression_origin, show_build,
};
use fwmap::core::model::{DwarfMode, SourceLinesMode};
use fwmap::core::rule_config::{apply_threshold_overrides, load_rule_config};
use fwmap::report::render::{SourceRenderOptions, write_html_report, write_json_report};
use rusqlite::{Connection, params};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::dto::{
    ActiveProjectStateDto, AnalysisRequestDto, ChangedFilesSummaryDto, CreateProjectRequestDto, DashboardQueryDto, DashboardSummaryDto, DeltaEntryDto, DesktopAppInfo,
    DesktopSettingsDto, ExportRequestDto, ExportResultDto, FirstRuleViolationSummaryDto, GitRefDto, HistoryItemDto, HistoryQueryDto, JobEventDto, JobStatusDto,
    MetricSummaryDto, OverviewCardDto, PolicyDocumentDto, PolicyValidationIssueDto, PolicyValidationResultDto, ProjectDetailDto, ProjectSummaryDto, RangeDiffQueryDto, RangeDiffResultDto, RecentExportDto, RecentRegressionDto, RegionUsageDto,
    RegressionOriginPointDto, RegressionQueryDto, RegressionResultDto, RegressionWindowRowDto, RunCompareRequestDto,
    RunCompareResultDto, RunDetailDto, RunSummaryDto, TimelineEntryDto, TimelineResultDto, TopGrowthEntryDto,
    TrendPointDto, TrendSeriesDto, UpdateProjectRequestDto, WorstCommitSummaryDto,
};
use crate::storage::{DesktopStorage, InsertExportRecord, InsertProjectRecord, InsertRunRecord, StoredProjectRecord, StoredRunRecord, UpdateProjectRecord};

#[derive(Debug, Clone)]
struct JobRecord {
    job_id: String,
    status: String,
    created_at: String,
    updated_at: String,
    label: Option<String>,
    progress_message: String,
    error_message: Option<String>,
    run_id: Option<i64>,
}

#[derive(Clone)]
pub struct DesktopState {
    storage: DesktopStorage,
    jobs: Arc<Mutex<HashMap<String, JobRecord>>>,
}

impl DesktopState {
    pub fn new(base_dir: impl AsRef<Path>) -> Result<Self, String> {
        Ok(Self {
            storage: DesktopStorage::new(base_dir)?,
            jobs: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn app_info(&self) -> Result<DesktopAppInfo, String> {
        Ok(DesktopAppInfo {
            app_name: "fwmap desktop".to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
            history_db_path: self.storage.paths().history_db_path.to_string_lossy().to_string(),
            app_db_path: self.storage.paths().app_db_path.to_string_lossy().to_string(),
        })
    }

    pub fn get_settings(&self) -> Result<DesktopSettingsDto, String> {
        self.storage.load_settings()
    }

    pub fn save_settings(&self, settings: DesktopSettingsDto) -> Result<DesktopSettingsDto, String> {
        self.storage.save_settings(&settings)?;
        self.storage.load_settings()
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectSummaryDto>, String> {
        Ok(self
            .storage
            .list_projects()?
            .into_iter()
            .map(|item| ProjectSummaryDto {
                project_id: item.project_id,
                name: item.name,
                root_path: item.root_path,
                git_repo_path: item.git_repo_path,
                default_rule_file_path: item.default_rule_file_path,
                default_target: item.default_target,
                default_profile: item.default_profile,
                last_run_at: item.last_run_at,
                last_export_at: item.last_export_at,
            })
            .collect())
    }

    pub fn create_project(&self, request: CreateProjectRequestDto) -> Result<ProjectDetailDto, String> {
        if request.name.trim().is_empty() {
            return Err("project name must not be empty".to_string());
        }
        if request.root_path.trim().is_empty() {
            return Err("project root path must not be empty".to_string());
        }
        let now = now_rfc3339();
        let project_id = self.storage.insert_project(&InsertProjectRecord {
            name: request.name,
            root_path: request.root_path,
            git_repo_path: request.git_repo_path,
            default_elf_path: request.default_elf_path,
            default_map_path: request.default_map_path,
            default_debug_path: request.default_debug_path,
            default_rule_file_path: request.default_rule_file_path,
            default_target: request.default_target,
            default_profile: request.default_profile,
            default_export_dir: request.default_export_dir,
            created_at: now.clone(),
            updated_at: now,
        })?;
        self.storage.set_active_project(Some(project_id))?;
        let project = self.storage.get_project(project_id)?.ok_or_else(|| "created project was not found".to_string())?;
        Ok(map_project_detail(project))
    }

    pub fn get_active_project(&self) -> Result<ActiveProjectStateDto, String> {
        let active_project_id = self.storage.get_active_project_id()?;
        let active_project = match active_project_id {
            Some(project_id) => self.storage.get_project(project_id)?.map(map_project_detail),
            None => None,
        };
        Ok(ActiveProjectStateDto { active_project_id, active_project })
    }

    pub fn set_active_project(&self, project_id: Option<i64>) -> Result<ActiveProjectStateDto, String> {
        if let Some(project_id) = project_id {
            if self.storage.get_project(project_id)?.is_none() {
                return Err(format!("project {project_id} was not found"));
            }
        }
        self.storage.set_active_project(project_id)?;
        self.get_active_project()
    }

    pub fn update_project(&self, project_id: i64, patch: UpdateProjectRequestDto) -> Result<ProjectDetailDto, String> {
        self.storage.update_project(project_id, &UpdateProjectRecord {
            name: patch.name,
            root_path: patch.root_path,
            git_repo_path: patch.git_repo_path,
            default_elf_path: patch.default_elf_path,
            default_map_path: patch.default_map_path,
            default_debug_path: patch.default_debug_path,
            default_rule_file_path: patch.default_rule_file_path,
            default_target: patch.default_target,
            default_profile: patch.default_profile,
            default_export_dir: patch.default_export_dir,
            pinned_report_path: patch.pinned_report_path,
            last_opened_screen: patch.last_opened_screen,
            last_opened_filters_json: patch.last_opened_filters_json,
            updated_at: now_rfc3339(),
        })?;
        let project = self.storage.get_project(project_id)?.ok_or_else(|| format!("project {project_id} was not found"))?;
        Ok(map_project_detail(project))
    }

    pub fn delete_project(&self, project_id: i64) -> Result<(), String> {
        self.storage.delete_project(project_id)
    }

    pub fn load_policy(&self, project_id: Option<i64>, path: Option<String>) -> Result<PolicyDocumentDto, String> {
        let project = match project_id {
            Some(project_id) => self.storage.get_project(project_id)?,
            None => self.storage.get_active_project_id().and_then(|id| match id {
                Some(project_id) => self.storage.get_project(project_id),
                None => Ok(None),
            })?,
        };
        let resolved_path = path
            .or_else(|| project.as_ref().and_then(|item| item.default_rule_file_path.clone()))
            .or_else(|| self.storage.load_settings().ok().and_then(|item| item.default_rule_file_path));
        let content = match resolved_path.as_ref() {
            Some(path) if Path::new(path).exists() => fs::read_to_string(path).map_err(|err| format!("failed to read policy '{}': {err}", path))?,
            _ => String::new(),
        };
        Ok(PolicyDocumentDto {
            path: resolved_path.clone(),
            format: policy_format(resolved_path.as_deref()),
            content,
            project_id: project.as_ref().map(|item| item.project_id),
        })
    }

    pub fn validate_policy(&self, document: PolicyDocumentDto) -> Result<PolicyValidationResultDto, String> {
        let mut issues = Vec::new();
        if document.content.trim().is_empty() {
            issues.push(PolicyValidationIssueDto {
                level: "error".to_string(),
                message: "policy content must not be empty".to_string(),
                line: None,
            });
        } else {
            match document.format.as_str() {
                "json" => {
                    if let Err(err) = serde_json::from_str::<serde_json::Value>(&document.content) {
                        issues.push(PolicyValidationIssueDto {
                            level: "error".to_string(),
                            message: format!("invalid JSON: {err}"),
                            line: None,
                        });
                    }
                }
                _ => {
                    if let Err(err) = toml::from_str::<toml::Value>(&document.content) {
                        issues.push(PolicyValidationIssueDto {
                            level: "error".to_string(),
                            message: format!("invalid TOML: {err}"),
                            line: None,
                        });
                    }
                }
            }
        }
        Ok(PolicyValidationResultDto { ok: issues.is_empty(), issues })
    }

    pub fn save_policy(&self, document: PolicyDocumentDto) -> Result<PolicyDocumentDto, String> {
        let validation = self.validate_policy(document.clone())?;
        if !validation.ok {
            return Err(validation.issues.first().map(|item| item.message.clone()).unwrap_or_else(|| "policy validation failed".to_string()));
        }
        let path = document.path.clone().ok_or_else(|| "policy path must be specified".to_string())?;
        if let Some(parent) = Path::new(&path).parent() {
            fs::create_dir_all(parent).map_err(|err| format!("failed to create policy directory '{}': {err}", parent.display()))?;
        }
        fs::write(&path, &document.content).map_err(|err| format!("failed to write policy '{}': {err}", path))?;
        if let Some(project_id) = document.project_id {
            self.storage.update_project(project_id, &UpdateProjectRecord {
                name: None,
                root_path: None,
                git_repo_path: None,
                default_elf_path: None,
                default_map_path: None,
                default_debug_path: None,
                default_rule_file_path: Some(path.clone()),
                default_target: None,
                default_profile: None,
                default_export_dir: None,
                pinned_report_path: None,
                last_opened_screen: Some("policy".to_string()),
                last_opened_filters_json: None,
                updated_at: now_rfc3339(),
            })?;
        }
        Ok(document)
    }

    pub fn export_report(&self, request: ExportRequestDto) -> Result<ExportResultDto, String> {
        if request.destination_path.trim().is_empty() {
            return Err("destination path must not be empty".to_string());
        }
        let destination = PathBuf::from(&request.destination_path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("failed to create export directory '{}': {err}", parent.display()))?;
        }
        let payload = self.build_export_payload(&request)?;
        match request.format.as_str() {
            "json" => fs::write(&destination, serde_json::to_string_pretty(&payload).map_err(|err| format!("failed to serialize export JSON: {err}"))?)
                .map_err(|err| format!("failed to write export '{}': {err}", destination.display()))?,
            "print-html" => fs::write(&destination, render_export_html(&payload, true))
                .map_err(|err| format!("failed to write export '{}': {err}", destination.display()))?,
            _ => fs::write(&destination, render_export_html(&payload, false))
                .map_err(|err| format!("failed to write export '{}': {err}", destination.display()))?,
        }
        let created_at = now_rfc3339();
        self.storage.insert_recent_export(&InsertExportRecord {
            project_id: request.project_id.or(self.storage.get_active_project_id()?),
            created_at: created_at.clone(),
            export_target: request.export_target.clone(),
            format: request.format.clone(),
            destination_path: request.destination_path.clone(),
            title: request.title.clone().unwrap_or_else(|| request.export_target.clone()),
        })?;
        Ok(ExportResultDto {
            destination_path: request.destination_path,
            export_target: request.export_target,
            format: request.format,
            created_at,
        })
    }

    pub fn list_recent_exports(&self, project_id: Option<i64>, limit: usize) -> Result<Vec<RecentExportDto>, String> {
        Ok(self.storage.list_recent_exports(project_id, limit)?.into_iter().map(|item| RecentExportDto {
            export_id: item.export_id,
            project_id: item.project_id,
            created_at: item.created_at,
            export_target: item.export_target,
            format: item.format,
            destination_path: item.destination_path,
            title: item.title,
        }).collect())
    }

    pub fn list_recent_runs(&self, limit: usize, offset: usize) -> Result<Vec<RunSummaryDto>, String> {
        self.storage.list_recent_runs(limit, offset)
    }

    pub fn run_detail(&self, run_id: i64) -> Result<Option<RunDetailDto>, String> {
        let Some(stored) = self.storage.get_recent_run(run_id)? else {
            return Ok(None);
        };
        Ok(Some(self.build_run_detail(stored)?))
    }

    pub fn dashboard_summary(&self, query: DashboardQueryDto) -> Result<DashboardSummaryDto, String> {
        let history_query = HistoryQueryDto {
            repo_path: query.repo_path.clone(),
            branch: query.branch.clone(),
            profile: query.profile.clone(),
            toolchain: query.toolchain.clone(),
            target: query.target.clone(),
            limit: Some(query.limit.unwrap_or(20).max(2)),
            order: Some("ancestry".to_string()),
        };
        let history_items = self.list_history(history_query.clone())?;
        let latest_history_item = history_items.first().cloned();
        let latest_run = self.storage.list_recent_runs(1, 0)?.into_iter().next();
        let recent_trends = build_dashboard_trends(&history_items);
        let top_growth_sources = if history_items.len() >= 2 {
            load_dashboard_top_growth(
                Path::new(&self.storage.load_settings()?.history_db_path),
                history_items[1].build_id,
                history_items[0].build_id,
            )?
        } else {
            Vec::new()
        };
        let region_usage = if let Some(item) = latest_history_item.as_ref() {
            show_build(Path::new(&self.storage.load_settings()?.history_db_path), item.build_id)?
                .map(|detail| {
                    detail
                        .regions
                        .into_iter()
                        .map(|(region_name, used_bytes, free_bytes, usage_ratio)| RegionUsageDto {
                            region_name,
                            used_bytes,
                            free_bytes,
                            usage_ratio,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let recent_regressions = build_recent_regressions(&history_items);
        let overview_cards = build_dashboard_cards(&history_items, latest_run.as_ref(), latest_history_item.as_ref(), &recent_regressions);
        Ok(DashboardSummaryDto {
            overview_cards,
            latest_run,
            latest_history_item,
            recent_trends,
            recent_regressions,
            top_growth_sources,
            region_usage,
        })
    }

    fn build_export_payload(&self, request: &ExportRequestDto) -> Result<serde_json::Value, String> {
        match request.export_target.as_str() {
            "run" => {
                let run_id = request.run_id.ok_or_else(|| "run export requires run_id".to_string())?;
                let detail = self.run_detail(run_id)?.ok_or_else(|| format!("run {run_id} was not found"))?;
                serde_json::to_value(detail).map_err(|err| format!("failed to serialize run export: {err}"))
            }
            "diff" => {
                let compare = request.compare.clone().ok_or_else(|| "diff export requires compare request".to_string())?;
                let result = self.compare_runs(compare)?;
                serde_json::to_value(result).map_err(|err| format!("failed to serialize diff export: {err}"))
            }
            "history" => {
                if let Some(range_query) = request.range_query.clone() {
                    let result = self.get_range_diff(range_query)?;
                    serde_json::to_value(result).map_err(|err| format!("failed to serialize history export: {err}"))
                } else {
                    let query = request.history_query.clone().unwrap_or_default();
                    let result = self.timeline(query)?;
                    serde_json::to_value(result).map_err(|err| format!("failed to serialize history export: {err}"))
                }
            }
            "regression" => {
                let query = request.regression_query.clone().ok_or_else(|| "regression export requires regression query".to_string())?;
                let result = self.detect_regression(query)?;
                serde_json::to_value(result).map_err(|err| format!("failed to serialize regression export: {err}"))
            }
            _ => {
                let query = request.dashboard_query.clone().unwrap_or_default();
                let result = self.dashboard_summary(query)?;
                serde_json::to_value(result).map_err(|err| format!("failed to serialize dashboard export: {err}"))
            }
        }
    }

    pub fn list_history(&self, query: HistoryQueryDto) -> Result<Vec<HistoryItemDto>, String> {
        let settings = self.storage.load_settings()?;
        let db_path = PathBuf::from(&settings.history_db_path);
        let mut builds = list_builds(&db_path)?;
        builds.retain(|build| matches_history_filters(build, &query));
        builds.sort_by(|a, b| b.created_at.cmp(&a.created_at).then_with(|| b.id.cmp(&a.id)));
        let limit = query.limit.unwrap_or(50);
        builds.truncate(limit);
        Ok(builds.into_iter().map(map_build_to_history_item).collect())
    }

    pub fn timeline(&self, query: HistoryQueryDto) -> Result<TimelineResultDto, String> {
        let settings = self.storage.load_settings()?;
        let db_path = PathBuf::from(&settings.history_db_path);
        let repo_path = resolve_repo_path(query.repo_path.as_deref(), &settings);
        let order = parse_commit_order(query.order.as_deref())?;
        let report = commit_timeline(
            &db_path,
            repo_path.as_deref(),
            query.branch.as_deref(),
            query.limit.unwrap_or(40),
            query.profile.as_deref(),
            query.toolchain.as_deref(),
            query.target.as_deref(),
            order,
        )?;
        Ok(TimelineResultDto {
            repo_id: report.repo_id,
            order: report.order,
            branch: report.filters.branch,
            profile: report.filters.profile,
            toolchain: report.filters.toolchain,
            target: report.filters.target,
            rows: report.rows.into_iter().map(map_timeline_row).collect(),
        })
    }

    pub fn compare_runs(&self, request: RunCompareRequestDto) -> Result<RunCompareResultDto, String> {
        let left = self
            .storage
            .get_recent_run(request.left_run_id)?
            .ok_or_else(|| format!("run {} was not found", request.left_run_id))?;
        let right = self
            .storage
            .get_recent_run(request.right_run_id)?
            .ok_or_else(|| format!("run {} was not found", request.right_run_id))?;
        if left.history_db_path != right.history_db_path {
            return Err("selected runs are stored in different history databases".to_string());
        }
        let db_path = PathBuf::from(&left.history_db_path);
        Ok(RunCompareResultDto {
            left_run: stored_run_summary(&left),
            right_run: stored_run_summary(&right),
            summary: MetricSummaryDto {
                rom_delta: right.rom_bytes as i64 - left.rom_bytes as i64,
                ram_delta: right.ram_bytes as i64 - left.ram_bytes as i64,
                warning_delta: right.warning_count as i64 - left.warning_count as i64,
            },
            region_deltas: load_metric_deltas(&db_path, left.build_id, right.build_id, MetricTable::Region, 8)?,
            section_deltas: load_metric_deltas(&db_path, left.build_id, right.build_id, MetricTable::Section, 10)?,
            object_deltas: load_metric_deltas(&db_path, left.build_id, right.build_id, MetricTable::Object, 10)?,
            source_file_deltas: load_metric_deltas(&db_path, left.build_id, right.build_id, MetricTable::SourceFile, 10)?,
            symbol_deltas: load_metric_deltas(&db_path, left.build_id, right.build_id, MetricTable::Symbol, 10)?,
            rust_dependency_deltas: load_metric_deltas(&db_path, left.build_id, right.build_id, MetricTable::RustDependency, 8)?,
            rust_family_deltas: load_metric_deltas(&db_path, left.build_id, right.build_id, MetricTable::RustFamily, 8)?,
        })
    }

    pub fn get_range_diff(&self, query: RangeDiffQueryDto) -> Result<RangeDiffResultDto, String> {
        let settings = self.storage.load_settings()?;
        let db_path = PathBuf::from(&settings.history_db_path);
        let repo_path = resolve_repo_path(query.repo_path.as_deref(), &settings);
        let order = parse_commit_order(query.order.as_deref())?;
        let report = range_diff(
            &db_path,
            repo_path.as_deref(),
            &query.spec,
            order,
            query.include_changed_files.unwrap_or(true),
            query.profile.as_deref(),
            query.toolchain.as_deref(),
            query.target.as_deref(),
        )?;
        Ok(RangeDiffResultDto {
            repo_id: report.repo_id,
            input_range_spec: report.input_range_spec,
            comparison_mode: report.comparison_mode,
            resolved_base: report.resolved_base,
            resolved_head: report.resolved_head,
            resolved_merge_base: report.resolved_merge_base,
            order: report.order,
            total_commits_in_git_range: report.total_commits_in_git_range,
            analyzed_commits_count: report.analyzed_commits_count,
            missing_analysis_commits_count: report.missing_analysis_commits_count,
            cumulative_rom_delta: report.cumulative_rom_delta,
            cumulative_ram_delta: report.cumulative_ram_delta,
            worst_commit_by_rom: report.worst_commit_by_rom.map(map_worst_commit),
            worst_commit_by_ram: report.worst_commit_by_ram.map(map_worst_commit),
            first_rule_violation: report.first_rule_violation.map(map_first_rule_violation),
            top_changed_sections: map_change_entries(report.top_changed_sections),
            top_changed_objects: map_change_entries(report.top_changed_objects),
            top_changed_source_files: map_change_entries(report.top_changed_source_files),
            top_changed_symbols: map_change_entries(report.top_changed_symbols),
            top_changed_rust_dependencies: map_change_entries(report.top_changed_rust_dependencies),
            top_changed_rust_families: map_change_entries(report.top_changed_rust_families),
            changed_files_summary: report.changed_files_summary.map(map_changed_files_summary),
            timeline_rows: report.timeline_rows.into_iter().map(map_timeline_row).collect(),
        })
    }

    pub fn detect_regression(&self, query: RegressionQueryDto) -> Result<RegressionResultDto, String> {
        let settings = self.storage.load_settings()?;
        let db_path = PathBuf::from(&settings.history_db_path);
        let repo_path = resolve_repo_path(query.repo_path.as_deref(), &settings);
        let order = parse_commit_order(query.order.as_deref())?;
        let detector = parse_regression_detector(&query.detector_type)?;
        let mode = parse_regression_mode(&query.mode)?;
        let report = regression_origin(
            &db_path,
            repo_path.as_deref(),
            &query.spec,
            detector,
            &query.key,
            mode,
            query.threshold,
            query.threshold_percent,
            query.jump_threshold,
            order,
            query.include_evidence.unwrap_or(true),
            query.include_changed_files.unwrap_or(true),
            query.bisect_like.unwrap_or(false),
            query.max_steps.unwrap_or(8),
            query.limit_commits,
            query.profile.as_deref(),
            query.toolchain.as_deref(),
            query.target.as_deref(),
        )?;
        let evidence = report.evidence.unwrap_or_default();
        Ok(RegressionResultDto {
            repo_id: report.repo_id,
            detector_type: detector_name(report.query.detector_type),
            key: report.query.key,
            mode: regression_mode_name(report.query.mode),
            confidence: regression_confidence_name(report.summary.confidence),
            reasoning: report.summary.reasoning,
            searched_commit_count: report.summary.searched_commit_count,
            analyzed_commit_count: report.summary.analyzed_commit_count,
            missing_analysis_count: report.summary.missing_analysis_count,
            mixed_configuration: report.summary.mixed_configuration,
            last_good: report.origin.last_good.map(map_regression_origin_point),
            first_observed_bad: report.origin.first_observed_bad.map(map_regression_origin_point),
            first_bad_candidate: report.origin.first_bad_candidate.map(map_regression_origin_point),
            transition_window: evidence.transition_window.into_iter().map(map_regression_window_row).collect(),
            top_growth_sections: map_change_entries(evidence.top_growth.sections),
            top_growth_objects: map_change_entries(evidence.top_growth.objects),
            top_growth_source_files: map_change_entries(evidence.top_growth.source_files),
            top_growth_symbols: map_change_entries(evidence.top_growth.symbols),
            changed_files_summary: evidence.changed_files.map(map_changed_files_summary),
            related_rule_hits: evidence.related_rule_hits,
            narrowed_commits: evidence.narrowed_commits,
        })
    }

    pub fn list_branches(&self, repo_path: Option<String>) -> Result<Vec<GitRefDto>, String> {
        let settings = self.storage.load_settings()?;
        let repo_path = resolve_repo_path(repo_path.as_deref(), &settings);
        list_git_refs(repo_path.as_deref(), "refs/heads", "branch")
    }

    pub fn list_tags(&self, repo_path: Option<String>) -> Result<Vec<GitRefDto>, String> {
        let settings = self.storage.load_settings()?;
        let repo_path = resolve_repo_path(repo_path.as_deref(), &settings);
        list_git_refs(repo_path.as_deref(), "refs/tags", "tag")
    }

    pub fn get_job_status(&self, job_id: &str) -> Result<Option<JobStatusDto>, String> {
        let jobs = self.jobs.lock().map_err(|_| "failed to access job state".to_string())?;
        Ok(jobs.get(job_id).cloned().map(into_job_status_dto))
    }

    pub fn cancel_job(&self, job_id: &str) -> Result<Option<JobStatusDto>, String> {
        let mut jobs = self.jobs.lock().map_err(|_| "failed to access job state".to_string())?;
        let Some(job) = jobs.get_mut(job_id) else {
            return Ok(None);
        };
        job.status = "cancel-requested".to_string();
        job.updated_at = now_rfc3339();
        job.progress_message = "Cancellation is not implemented in Phase D2".to_string();
        Ok(Some(into_job_status_dto(job.clone())))
    }

    pub fn start_analysis(&self, app: AppHandle, request: AnalysisRequestDto) -> Result<JobStatusDto, String> {
        let settings = self.storage.load_settings()?;
        let active_project = self.get_active_project()?.active_project;
        let request = apply_project_defaults(request, active_project.as_ref());
        self.storage
            .remember_selected_files(request.elf_path.as_deref(), request.map_path.as_deref())?;

        let job_id = Uuid::new_v4().to_string();
        let now = now_rfc3339();
        let job = JobRecord {
            job_id: job_id.clone(),
            status: "queued".to_string(),
            created_at: now.clone(),
            updated_at: now,
            label: request.label.clone(),
            progress_message: "Queued".to_string(),
            error_message: None,
            run_id: None,
        };
        {
            let mut jobs = self.jobs.lock().map_err(|_| "failed to access job state".to_string())?;
            jobs.insert(job_id.clone(), job.clone());
        }
        emit_event(&app, "job-created", &job, None, None);

        let state = self.clone();
        std::thread::spawn(move || {
            let result = state.run_analysis(&app, &settings, request, &job_id);
            if let Err(err) = result {
                let _ = state.update_job(&job_id, "failed", "Analysis failed", Some(err.clone()), None);
                if let Ok(Some(job)) = state.get_job_status(&job_id) {
                    let payload = JobEventDto {
                        job_id: job.job_id,
                        status: job.status,
                        message: job.progress_message,
                        run_id: job.run_id,
                        error_message: Some(err),
                    };
                    let _ = app.emit("job-failed", payload);
                }
            }
        });

        Ok(into_job_status_dto(job))
    }

    fn run_analysis(
        &self,
        app: &AppHandle,
        settings: &DesktopSettingsDto,
        request: AnalysisRequestDto,
        job_id: &str,
    ) -> Result<(), String> {
        let active_project = self.get_active_project()?.active_project;
        let elf_path = request
            .elf_path
            .as_deref()
            .ok_or_else(|| "elfPath is required".to_string())?;
        let elf_path = PathBuf::from(elf_path);
        if !elf_path.is_file() {
            return Err(format!("ELF file was not found: {}", elf_path.display()));
        }
        let map_path = request.map_path.as_deref().map(PathBuf::from);
        if let Some(path) = map_path.as_ref() {
            if !path.is_file() {
                return Err(format!("map file was not found: {}", path.display()));
            }
        }

        self.update_job(job_id, "running", "Preparing analysis", None, None)?;
        emit_status(app, "job-progress", self, job_id)?;

        let mut options = AnalyzeOptions {
            dwarf_mode: DwarfMode::Auto,
            source_lines: SourceLinesMode::Off,
            git: GitOptions {
                enabled: request.git_repo_path.is_some() || settings.default_git_repo_path.is_some(),
                repo_path: request
                    .git_repo_path
                    .as_ref()
                    .map(PathBuf::from)
                    .or_else(|| settings.default_git_repo_path.as_ref().map(PathBuf::from)),
            },
            ..AnalyzeOptions::default()
        };

        if let Some(rule_path) = request
            .rule_file_path
            .as_ref()
            .or(settings.default_rule_file_path.as_ref())
            .map(PathBuf::from)
        {
            let config = load_rule_config(&rule_path)?;
            apply_threshold_overrides(&mut options.thresholds, &config.thresholds);
            options.custom_rules = config.rules;
        }

        self.update_job(job_id, "running", "Analyzing ELF and map data", None, None)?;
        emit_status(app, "job-progress", self, job_id)?;
        let analysis = analyze_paths(&elf_path, map_path.as_deref(), None, &options)?;

        let history_db = PathBuf::from(&settings.history_db_path);
        if let Some(parent) = history_db.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create history dir '{}': {err}", parent.display()))?;
        }
        let mut metadata = BTreeMap::new();
        if let Some(label) = request.label.as_ref() {
            metadata.insert("desktop.label".to_string(), label.clone());
        }
        metadata.insert(
            "toolchain.id".to_string(),
            analysis
                .toolchain
                .detected
                .map(|item| item.to_string())
                .unwrap_or_else(|| analysis.toolchain.resolved.to_string()),
        );
        metadata.insert(
            "config.fingerprint".to_string(),
            format!(
                "{}|{}|{}",
                analysis.toolchain.linker_family, analysis.toolchain.map_format, analysis.debug_info.source_lines
            ),
        );
        if let Some(rust) = analysis.rust_context.as_ref() {
            if let Some(target) = rust.target_triple.as_ref().or(rust.target_name.as_ref()) {
                metadata.insert("target.id".to_string(), target.clone());
            }
            if let Some(profile) = rust.profile.as_ref() {
                metadata.insert("build.profile".to_string(), profile.clone());
            }
        }

        self.update_job(job_id, "running", "Saving history and reports", None, None)?;
        emit_status(app, "job-progress", self, job_id)?;
        let build_id = record_build(&history_db, HistoryRecordInput { analysis: analysis.clone(), metadata })?;
        let run_dir = self.storage.paths().runs_dir.join(job_id);
        std::fs::create_dir_all(&run_dir)
            .map_err(|err| format!("failed to create run dir '{}': {err}", run_dir.display()))?;
        let report_html_path = run_dir.join("report.html");
        let report_json_path = run_dir.join("report.json");
        write_html_report(&report_html_path, &analysis, None, SourceRenderOptions::default(), 3)?;
        write_json_report(
            &report_json_path,
            &analysis,
            None,
            &options.thresholds,
            SourceRenderOptions::default(),
            3,
        )?;

        let build = list_builds(&history_db)?
            .into_iter()
            .find(|item| item.id == build_id)
            .ok_or_else(|| format!("failed to load recorded build #{build_id}"))?;

        let run_id = self.storage.insert_recent_run(&InsertRunRecord {
            project_id: active_project.as_ref().map(|item| item.project_id),
            build_id,
            created_at: now_rfc3339(),
            label: request.label,
            status: "completed".to_string(),
            git_revision: build.git.as_ref().map(|item| item.short_commit_hash.clone()),
            profile: build
                .rust_context
                .as_ref()
                .and_then(|item| item.profile.clone())
                .or_else(|| build.metadata.get("build.profile").cloned()),
            target: build
                .rust_context
                .as_ref()
                .and_then(|item| item.target_triple.clone().or(item.target_name.clone()))
                .or_else(|| build.metadata.get("target.id").cloned()),
            rom_bytes: build.rom_bytes,
            ram_bytes: build.ram_bytes,
            warning_count: build.warning_count,
            history_db_path: history_db.to_string_lossy().to_string(),
            report_html_path: Some(report_html_path.to_string_lossy().to_string()),
            report_json_path: Some(report_json_path.to_string_lossy().to_string()),
        })?;

        self.update_job(job_id, "finished", "Analysis finished", None, Some(run_id))?;
        if let Some(job) = self.get_job_status(job_id)? {
            let payload = JobEventDto {
                job_id: job.job_id,
                status: job.status,
                message: job.progress_message,
                run_id: job.run_id,
                error_message: None,
            };
            let _ = app.emit("job-finished", payload);
        }
        Ok(())
    }

    fn update_job(
        &self,
        job_id: &str,
        status: &str,
        message: &str,
        error_message: Option<String>,
        run_id: Option<i64>,
    ) -> Result<(), String> {
        let mut jobs = self.jobs.lock().map_err(|_| "failed to access job state".to_string())?;
        let Some(job) = jobs.get_mut(job_id) else {
            return Err(format!("unknown job id '{job_id}'"));
        };
        job.status = status.to_string();
        job.updated_at = now_rfc3339();
        job.progress_message = message.to_string();
        if error_message.is_some() {
            job.error_message = error_message;
        }
        if run_id.is_some() {
            job.run_id = run_id;
        }
        Ok(())
    }

    fn build_run_detail(&self, stored: StoredRunRecord) -> Result<RunDetailDto, String> {
        let summary = stored_run_summary(&stored);
        let detail = show_build(Path::new(&stored.history_db_path), stored.build_id)?;
        let Some(detail) = detail else {
            return Ok(RunDetailDto {
                run: summary,
                elf_path: String::new(),
                arch: String::new(),
                linker_family: String::new(),
                map_format: String::new(),
                report_html_path: stored.report_html_path,
                report_json_path: stored.report_json_path,
                git_branch: None,
                git_describe: None,
                top_sections: Vec::new(),
                top_symbols: Vec::new(),
                warnings: Vec::new(),
            });
        };

        Ok(RunDetailDto {
            run: summary,
            elf_path: detail.build.elf_path,
            arch: detail.build.arch,
            linker_family: detail.build.linker_family,
            map_format: detail.build.map_format,
            report_html_path: stored.report_html_path,
            report_json_path: stored.report_json_path,
            git_branch: detail.build.git.as_ref().and_then(|item| item.branch_name.clone()),
            git_describe: detail.build.git.as_ref().and_then(|item| item.describe.clone()),
            top_sections: detail.top_sections,
            top_symbols: detail.top_functions.into_iter().map(|(name, _, size)| (name, size)).collect(),
            warnings: detail.warnings,
        })
    }
}

#[derive(Clone, Copy)]
enum MetricTable {
    Region,
    Section,
    Object,
    SourceFile,
    Symbol,
    RustDependency,
    RustFamily,
}

fn stored_run_summary(stored: &StoredRunRecord) -> RunSummaryDto {
    RunSummaryDto {
        run_id: stored.run_id,
        build_id: stored.build_id,
        created_at: stored.created_at.clone(),
        label: stored.label.clone(),
        status: stored.status.clone(),
        git_revision: stored.git_revision.clone(),
        profile: stored.profile.clone(),
        target: stored.target.clone(),
        rom_bytes: stored.rom_bytes,
        ram_bytes: stored.ram_bytes,
        warning_count: stored.warning_count,
    }
}

fn map_build_to_history_item(build: fwmap::core::history::BuildRecord) -> HistoryItemDto {
    HistoryItemDto {
        build_id: build.id,
        created_at: unix_to_rfc3339(build.created_at),
        elf_path: build.elf_path,
        arch: build.arch,
        linker_family: build.linker_family,
        map_format: build.map_format,
        rom_bytes: build.rom_bytes,
        ram_bytes: build.ram_bytes,
        warning_count: build.warning_count,
        error_count: build.error_count,
        git_revision: build.git.as_ref().map(|item| item.short_commit_hash.clone()),
        git_branch: build.git.as_ref().and_then(|item| item.branch_name.clone()),
        git_subject: build.git.as_ref().and_then(|item| item.commit_subject.clone()),
        git_describe: build.git.as_ref().and_then(|item| item.describe.clone()),
        profile: build.metadata.get("build.profile").cloned(),
        target: build.metadata.get("target.id").cloned(),
        toolchain_id: build.metadata.get("toolchain.id").cloned(),
        label: build.metadata.get("desktop.label").cloned(),
    }
}

fn matches_history_filters(build: &fwmap::core::history::BuildRecord, query: &HistoryQueryDto) -> bool {
    if let Some(branch) = query.branch.as_deref() {
        if build.git.as_ref().and_then(|item| item.branch_name.as_deref()) != Some(branch) {
            return false;
        }
    }
    if let Some(profile) = query.profile.as_deref() {
        if build.metadata.get("build.profile").map(String::as_str) != Some(profile) {
            return false;
        }
    }
    if let Some(toolchain) = query.toolchain.as_deref() {
        if build.metadata.get("toolchain.id").map(String::as_str) != Some(toolchain) {
            return false;
        }
    }
    if let Some(target) = query.target.as_deref() {
        if build.metadata.get("target.id").map(String::as_str) != Some(target) {
            return false;
        }
    }
    if let Some(repo_path) = query.repo_path.as_deref() {
        if build.git.as_ref().map(|item| item.repo_root.as_str()) != Some(repo_path) {
            return false;
        }
    }
    true
}

fn parse_commit_order(value: Option<&str>) -> Result<CommitOrder, String> {
    match value.unwrap_or("ancestry") {
        "ancestry" => Ok(CommitOrder::Ancestry),
        "timestamp" => Ok(CommitOrder::Timestamp),
        other => Err(format!("unsupported commit order '{other}', expected ancestry or timestamp")),
    }
}

fn parse_regression_detector(value: &str) -> Result<RegressionDetector, String> {
    match value {
        "metric" => Ok(RegressionDetector::Metric),
        "rule" => Ok(RegressionDetector::Rule),
        "entity" => Ok(RegressionDetector::Entity),
        other => Err(format!("unsupported regression detector '{other}'")),
    }
}

fn parse_regression_mode(value: &str) -> Result<RegressionMode, String> {
    match value {
        "first-crossing" => Ok(RegressionMode::FirstCrossing),
        "first-jump" => Ok(RegressionMode::FirstJump),
        "first-presence" => Ok(RegressionMode::FirstPresence),
        "first-violation" => Ok(RegressionMode::FirstViolation),
        other => Err(format!("unsupported regression mode '{other}'")),
    }
}

fn resolve_repo_path(repo_path: Option<&str>, settings: &DesktopSettingsDto) -> Option<PathBuf> {
    repo_path
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| settings.default_git_repo_path.as_ref().map(PathBuf::from))
}

fn list_git_refs(repo_path: Option<&Path>, namespace: &str, kind: &str) -> Result<Vec<GitRefDto>, String> {
    let mut command = Command::new("git");
    if let Some(path) = repo_path {
        command.arg("-C").arg(path);
    }
    let output = command
        .args(["for-each-ref", namespace, "--format=%(refname:short)"])
        .output()
        .map_err(|err| format!("failed to run git for-each-ref: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("git for-each-ref failed with status {}", output.status)
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| GitRefDto {
            name: line.to_string(),
            kind: kind.to_string(),
        })
        .collect())
}

fn load_metric_deltas(
    db_path: &Path,
    left_build_id: i64,
    right_build_id: i64,
    metric: MetricTable,
    limit: usize,
) -> Result<Vec<DeltaEntryDto>, String> {
    let conn = Connection::open(db_path)
        .map_err(|err| format!("failed to open history database '{}': {err}", db_path.display()))?;
    let (current, previous) = match metric {
        MetricTable::Region => (
            load_metric_map(&conn, "region_metrics", "region_name", "used_bytes", right_build_id)?,
            load_metric_map(&conn, "region_metrics", "region_name", "used_bytes", left_build_id)?,
        ),
        MetricTable::Section => (
            load_metric_map(&conn, "section_metrics", "section_name", "size_bytes", right_build_id)?,
            load_metric_map(&conn, "section_metrics", "section_name", "size_bytes", left_build_id)?,
        ),
        MetricTable::Object => (
            load_metric_map(&conn, "object_metrics", "object_path", "size_bytes", right_build_id)?,
            load_metric_map(&conn, "object_metrics", "object_path", "size_bytes", left_build_id)?,
        ),
        MetricTable::SourceFile => (
            load_metric_map(&conn, "source_file_metrics", "path", "size_bytes", right_build_id)?,
            load_metric_map(&conn, "source_file_metrics", "path", "size_bytes", left_build_id)?,
        ),
        MetricTable::Symbol => (
            load_metric_map(&conn, "symbol_metrics", "name", "size_bytes", right_build_id)?,
            load_metric_map(&conn, "symbol_metrics", "name", "size_bytes", left_build_id)?,
        ),
        MetricTable::RustDependency => (
            load_scoped_metric_map(&conn, "dependency", right_build_id)?,
            load_scoped_metric_map(&conn, "dependency", left_build_id)?,
        ),
        MetricTable::RustFamily => (
            load_like_scoped_metric_map(&conn, "family:%", right_build_id)?,
            load_like_scoped_metric_map(&conn, "family:%", left_build_id)?,
        ),
    };
    Ok(diff_metric_maps(current, previous, limit))
}

fn load_metric_map(
    conn: &Connection,
    table: &str,
    key_column: &str,
    value_column: &str,
    build_id: i64,
) -> Result<BTreeMap<String, i64>, String> {
    let sql = format!("SELECT {key_column}, {value_column} FROM {table} WHERE build_id = ?1");
    let mut stmt = conn.prepare(&sql).map_err(|err| format!("failed to prepare metric query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|err| format!("failed to query metric rows: {err}"))?;
    let pairs = rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to read metric rows: {err}"))?;
    Ok(pairs.into_iter().collect())
}

fn load_scoped_metric_map(conn: &Connection, scope: &str, build_id: i64) -> Result<BTreeMap<String, i64>, String> {
    let mut stmt = conn
        .prepare("SELECT name, size_bytes FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope = ?2")
        .map_err(|err| format!("failed to prepare rust metric query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id, scope], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|err| format!("failed to query rust metric rows: {err}"))?;
    let pairs = rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to read rust metric rows: {err}"))?;
    Ok(pairs.into_iter().collect())
}

fn load_like_scoped_metric_map(conn: &Connection, scope_like: &str, build_id: i64) -> Result<BTreeMap<String, i64>, String> {
    let mut stmt = conn
        .prepare("SELECT name, size_bytes FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope LIKE ?2")
        .map_err(|err| format!("failed to prepare rust metric query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id, scope_like], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|err| format!("failed to query rust metric rows: {err}"))?;
    let pairs = rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to read rust metric rows: {err}"))?;
    Ok(pairs.into_iter().collect())
}

fn diff_metric_maps(current: BTreeMap<String, i64>, previous: BTreeMap<String, i64>, limit: usize) -> Vec<DeltaEntryDto> {
    let mut names = current.keys().chain(previous.keys()).cloned().collect::<Vec<_>>();
    names.sort();
    names.dedup();
    let mut entries = names
        .into_iter()
        .filter_map(|name| {
            let delta = current.get(&name).copied().unwrap_or_default() - previous.get(&name).copied().unwrap_or_default();
            (delta != 0).then_some(DeltaEntryDto { name, delta })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| b.delta.abs().cmp(&a.delta.abs()).then_with(|| a.name.cmp(&b.name)));
    entries.truncate(limit);
    entries
}

fn map_timeline_row(row: fwmap::core::history::CommitTimelineRow) -> TimelineEntryDto {
    TimelineEntryDto {
        commit: row.commit,
        short_commit: row.short_commit,
        commit_time: row.commit_time,
        author_name: row.author_name,
        subject: row.subject,
        branch_names: row.branch_names,
        tag_names: row.tag_names,
        describe: row.describe,
        build_profile: row.build_profile,
        toolchain_id: row.toolchain_id,
        target_id: row.target_id,
        rom_total: row.rom_total,
        ram_total: row.ram_total,
        rom_delta_vs_previous: row.rom_delta_vs_previous,
        ram_delta_vs_previous: row.ram_delta_vs_previous,
        rule_violations_count: row.rule_violations_count,
        top_sections: map_change_entries(row.top_increases.sections),
        top_objects: map_change_entries(row.top_increases.objects),
        top_source_files: map_change_entries(row.top_increases.source_files),
        top_symbols: map_change_entries(row.top_increases.symbols),
    }
}

fn map_change_entries(items: Vec<fwmap::core::history::ChangeEntry>) -> Vec<DeltaEntryDto> {
    items
        .into_iter()
        .map(|item| DeltaEntryDto {
            name: item.name,
            delta: item.delta,
        })
        .collect()
}

fn map_worst_commit(item: fwmap::core::history::WorstCommitSummary) -> WorstCommitSummaryDto {
    WorstCommitSummaryDto {
        commit: item.commit,
        delta: item.delta,
        subject: item.subject,
        date: item.date,
    }
}

fn map_first_rule_violation(item: fwmap::core::history::FirstRuleViolationSummary) -> FirstRuleViolationSummaryDto {
    FirstRuleViolationSummaryDto {
        commit: item.commit,
        rule_ids: item.rule_ids,
        subject: item.subject,
    }
}

fn map_changed_files_summary(item: fwmap::core::history::ChangedFilesSummary) -> ChangedFilesSummaryDto {
    ChangedFilesSummaryDto {
        git_changed_files: item.git_changed_files,
        changed_source_files_in_analysis: item.changed_source_files_in_analysis,
        intersection_files: item.intersection_files,
        git_only_files_count: item.git_only_files_count,
        analysis_only_files_count: item.analysis_only_files_count,
        intersection_count: item.intersection_count,
    }
}

fn map_regression_origin_point(item: fwmap::core::history::RegressionOriginPoint) -> RegressionOriginPointDto {
    RegressionOriginPointDto {
        commit: item.commit,
        short_commit: item.short_commit,
        subject: item.subject,
        value: item.value,
    }
}

fn map_regression_window_row(item: fwmap::core::history::RegressionWindowRow) -> RegressionWindowRowDto {
    RegressionWindowRowDto {
        commit: item.commit,
        short_commit: item.short_commit,
        subject: item.subject,
        status: item.status,
        value: item.value,
    }
}

fn detector_name(value: RegressionDetector) -> String {
    match value {
        RegressionDetector::Metric => "metric",
        RegressionDetector::Rule => "rule",
        RegressionDetector::Entity => "entity",
    }
    .to_string()
}

fn regression_mode_name(value: RegressionMode) -> String {
    match value {
        RegressionMode::FirstCrossing => "first-crossing",
        RegressionMode::FirstJump => "first-jump",
        RegressionMode::FirstPresence => "first-presence",
        RegressionMode::FirstViolation => "first-violation",
    }
    .to_string()
}

fn regression_confidence_name(value: RegressionConfidence) -> String {
    match value {
        RegressionConfidence::High => "high",
        RegressionConfidence::Medium => "medium",
        RegressionConfidence::Low => "low",
        RegressionConfidence::Unknown => "unknown",
    }
    .to_string()
}

fn unix_to_rfc3339(value: i64) -> String {
    Utc.timestamp_opt(value, 0)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| value.to_string())
}

fn into_job_status_dto(job: JobRecord) -> JobStatusDto {
    JobStatusDto {
        job_id: job.job_id,
        status: job.status,
        created_at: job.created_at,
        updated_at: job.updated_at,
        label: job.label,
        progress_message: job.progress_message,
        error_message: job.error_message,
        run_id: job.run_id,
    }
}

fn emit_event(app: &AppHandle, name: &str, job: &JobRecord, run_id: Option<i64>, error_message: Option<String>) {
    let payload = JobEventDto {
        job_id: job.job_id.clone(),
        status: job.status.clone(),
        message: job.progress_message.clone(),
        run_id: run_id.or(job.run_id),
        error_message,
    };
    let _ = app.emit(name, payload);
}

fn emit_status(app: &AppHandle, event_name: &str, state: &DesktopState, job_id: &str) -> Result<(), String> {
    if let Some(job) = state.get_job_status(job_id)? {
        let payload = JobEventDto {
            job_id: job.job_id,
            status: job.status,
            message: job.progress_message,
            run_id: job.run_id,
            error_message: job.error_message,
        };
        let _ = app.emit(event_name, payload);
    }
    Ok(())
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}


fn build_dashboard_cards(
    history_items: &[HistoryItemDto],
    latest_run: Option<&RunSummaryDto>,
    latest_history_item: Option<&HistoryItemDto>,
    recent_regressions: &[RecentRegressionDto],
) -> Vec<OverviewCardDto> {
    let latest = latest_history_item.or_else(|| history_items.first());
    vec![
        OverviewCardDto {
            key: "latest-run".to_string(),
            title: "Latest Run".to_string(),
            value: latest_run
                .map(|item| item.label.clone().unwrap_or_else(|| format!("#{}", item.run_id)))
                .unwrap_or_else(|| "-".to_string()),
            subtitle: latest_run.map(|item| format_time_short(&item.created_at)),
            tone: "neutral".to_string(),
        },
        OverviewCardDto {
            key: "latest-branch".to_string(),
            title: "Current Branch".to_string(),
            value: latest.and_then(|item| item.git_branch.clone()).unwrap_or_else(|| "-".to_string()),
            subtitle: latest.and_then(|item| item.git_revision.clone()),
            tone: "info".to_string(),
        },
        OverviewCardDto {
            key: "latest-rom".to_string(),
            title: "Latest ROM".to_string(),
            value: latest.map(|item| format_bytes_compact(item.rom_bytes)).unwrap_or_else(|| "-".to_string()),
            subtitle: latest.map(|item| format!("RAM {}", format_bytes_compact(item.ram_bytes))),
            tone: "primary".to_string(),
        },
        OverviewCardDto {
            key: "warnings".to_string(),
            title: "Rule Violations".to_string(),
            value: latest.map(|item| item.warning_count.to_string()).unwrap_or_else(|| "0".to_string()),
            subtitle: latest.map(|item| format!("errors {}", item.error_count)),
            tone: if latest.map(|item| item.warning_count > 0).unwrap_or(false) { "warning" } else { "success" }.to_string(),
        },
        OverviewCardDto {
            key: "recent-regressions".to_string(),
            title: "Recent Regressions".to_string(),
            value: recent_regressions.len().to_string(),
            subtitle: recent_regressions.first().map(|item| item.commit.clone()),
            tone: if recent_regressions.is_empty() { "success" } else { "danger" }.to_string(),
        },
    ]
}

fn build_dashboard_trends(history_items: &[HistoryItemDto]) -> Vec<TrendSeriesDto> {
    let ordered = history_items.iter().rev().collect::<Vec<_>>();
    vec![
        TrendSeriesDto {
            key: "rom-ram".to_string(),
            label: "ROM / RAM".to_string(),
            unit: "bytes".to_string(),
            points: ordered
                .iter()
                .map(|item| TrendPointDto {
                    label: item.git_revision.clone().unwrap_or_else(|| format!("#{}", item.build_id)),
                    value: item.rom_bytes as f64,
                    secondary_value: Some(item.ram_bytes as f64),
                })
                .collect(),
        },
        TrendSeriesDto {
            key: "warnings".to_string(),
            label: "Rule Violations".to_string(),
            unit: "count".to_string(),
            points: ordered
                .iter()
                .map(|item| TrendPointDto {
                    label: item.git_revision.clone().unwrap_or_else(|| format!("#{}", item.build_id)),
                    value: item.warning_count as f64,
                    secondary_value: Some(item.error_count as f64),
                })
                .collect(),
        },
    ]
}

fn build_recent_regressions(history_items: &[HistoryItemDto]) -> Vec<RecentRegressionDto> {
    history_items
        .iter()
        .filter(|item| item.warning_count > 0)
        .take(5)
        .map(|item| RecentRegressionDto {
            detector_type: "rule".to_string(),
            key: item.git_subject.clone().unwrap_or_else(|| "warnings-present".to_string()),
            confidence: "medium".to_string(),
            commit: item.git_revision.clone().unwrap_or_else(|| format!("#{}", item.build_id)),
            subject: item.git_subject.clone().unwrap_or_else(|| item.elf_path.clone()),
            reasoning: format!("{} rule warnings were recorded for this build", item.warning_count),
        })
        .collect()
}

fn load_dashboard_top_growth(db_path: &Path, left_build_id: i64, right_build_id: i64) -> Result<Vec<TopGrowthEntryDto>, String> {
    let mut entries = Vec::new();
    for item in load_metric_deltas(db_path, left_build_id, right_build_id, MetricTable::Section, 4)? {
        entries.push(TopGrowthEntryDto { scope: "section".to_string(), name: item.name, delta: item.delta, detail: None });
    }
    for item in load_metric_deltas(db_path, left_build_id, right_build_id, MetricTable::SourceFile, 4)? {
        entries.push(TopGrowthEntryDto { scope: "source".to_string(), name: item.name, delta: item.delta, detail: None });
    }
    for item in load_metric_deltas(db_path, left_build_id, right_build_id, MetricTable::Symbol, 4)? {
        entries.push(TopGrowthEntryDto { scope: "symbol".to_string(), name: item.name, delta: item.delta, detail: None });
    }
    entries.sort_by(|a, b| b.delta.abs().cmp(&a.delta.abs()).then_with(|| a.name.cmp(&b.name)));
    entries.truncate(8);
    Ok(entries)
}

fn format_bytes_compact(value: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut size = value as f64;
    let mut unit_index = 0usize;
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    if unit_index == 0 || size >= 100.0 {
        format!("{size:.0} {}", UNITS[unit_index])
    } else {
        format!("{size:.1} {}", UNITS[unit_index])
    }
}

fn format_time_short(value: &str) -> String {
    value.chars().take(16).collect()
}



fn map_project_detail(item: StoredProjectRecord) -> ProjectDetailDto {
    ProjectDetailDto {
        project_id: item.project_id,
        name: item.name,
        root_path: item.root_path,
        git_repo_path: item.git_repo_path,
        default_elf_path: item.default_elf_path,
        default_map_path: item.default_map_path,
        default_debug_path: item.default_debug_path,
        default_rule_file_path: item.default_rule_file_path,
        default_target: item.default_target,
        default_profile: item.default_profile,
        default_export_dir: item.default_export_dir,
        pinned_report_path: item.pinned_report_path,
        last_opened_screen: item.last_opened_screen,
        last_opened_filters_json: item.last_opened_filters_json,
        created_at: item.created_at,
        updated_at: item.updated_at,
    }
}

fn apply_project_defaults(mut request: AnalysisRequestDto, project: Option<&ProjectDetailDto>) -> AnalysisRequestDto {
    if let Some(project) = project {
        if request.elf_path.is_none() {
            request.elf_path = project.default_elf_path.clone();
        }
        if request.map_path.is_none() {
            request.map_path = project.default_map_path.clone();
        }
        if request.debug_path.is_none() {
            request.debug_path = project.default_debug_path.clone();
        }
        if request.rule_file_path.is_none() {
            request.rule_file_path = project.default_rule_file_path.clone();
        }
        if request.git_repo_path.is_none() {
            request.git_repo_path = project.git_repo_path.clone();
        }
    }
    request
}

fn policy_format(path: Option<&str>) -> String {
    match path.and_then(|item| Path::new(item).extension()).and_then(|item| item.to_str()) {
        Some("json") => "json".to_string(),
        _ => "toml".to_string(),
    }
}

fn render_export_html(payload: &serde_json::Value, print_friendly: bool) -> String {
    let body = serde_json::to_string_pretty(payload).unwrap_or_else(|_| "{}".to_string());
    let extra = if print_friendly {
        "@media print { body { background: white; color: black; } pre { white-space: pre-wrap; } }"
    } else {
        ""
    };
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>fwmap export</title><style>body{{font-family:Segoe UI,Noto Sans JP,sans-serif;background:#0f1726;color:#e5eefc;padding:24px;}}pre{{background:#111827;padding:16px;border-radius:16px;overflow:auto;}}{extra}</style></head><body><h1>fwmap export</h1><pre>{}</pre></body></html>",
        html_escape(&body)
    )
}

fn html_escape(input: &str) -> String {
    input.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
