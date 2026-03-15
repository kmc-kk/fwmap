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
use rusqlite::{Connection, OptionalExtension, params};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::dto::{
    ActiveProjectStateDto, AnalysisRequestDto, ChangedFilesSummaryDto, CreateInvestigationPackageRequestDto, CreateProjectRequestDto, DashboardQueryDto, DashboardSummaryDto, DeltaEntryDto, DesktopAppInfo,
    DesktopSettingsDto, ExportRequestDto, ExportResultDto, ExtensionPointDto, FirstRuleViolationSummaryDto, GitRefDto, HistoryItemDto, HistoryQueryDto, InspectorBreakdownDto, InspectorDetailDto, InspectorHierarchyNodeDto, InspectorItemDto, InspectorQueryDto, InspectorSelectionDto, InspectorSummaryDto, InvestigationPackageItemDto, InvestigationPackageManifestDto, InvestigationPackageSummaryDto, JobEventDto, JobStatusDto,
    MetricSummaryDto, OpenInvestigationPackageResultDto, OverviewCardDto, PluginCapabilityDto, PluginDetailDto, PluginExecutionRequestDto, PluginExecutionResultDto, PluginOutputItemDto, PluginSummaryDto, PolicyDocumentDto, PolicyValidationIssueDto, PolicyValidationResultDto, ProjectDetailDto, ProjectSummaryDto, RangeDiffQueryDto, RangeDiffResultDto, RecentExportDto, RecentRegressionDto, RegionUsageDto,
    RegressionOriginPointDto, RegressionQueryDto, RegressionResultDto, RegressionWindowRowDto, RunCompareRequestDto,
    RunCompareResultDto, RunDetailDto, RunSummaryDto, SourceContextDto, TimelineEntryDto, TimelineResultDto, TopGrowthEntryDto,
    TrendPointDto, TrendSeriesDto, UpdateProjectRequestDto, WorstCommitSummaryDto,
};
use crate::storage::{DesktopStorage, InsertExportRecord, InsertPackageRecord, InsertPluginStateRecord, InsertProjectRecord, InsertRunRecord, StoredPackageRecord, StoredProjectRecord, StoredRunRecord, UpdateProjectRecord};

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

const INVESTIGATION_PACKAGE_SCHEMA_VERSION: i64 = 1;

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

    pub fn list_extension_points(&self) -> Result<Vec<ExtensionPointDto>, String> {
        Ok(built_in_extension_points())
    }

    pub fn list_plugins(&self) -> Result<Vec<PluginSummaryDto>, String> {
        let stored = self.storage.list_plugin_states()?;
        Ok(built_in_plugin_summaries(&stored))
    }

    pub fn get_plugin_detail(&self, plugin_id: &str) -> Result<PluginDetailDto, String> {
        let stored = self.storage.get_plugin_state(plugin_id)?;
        plugin_detail(plugin_id, stored.as_ref()).ok_or_else(|| format!("plugin '{plugin_id}' was not found"))
    }

    pub fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) -> Result<PluginSummaryDto, String> {
        let detail = self.get_plugin_detail(plugin_id)?;
        self.storage.save_plugin_state(&InsertPluginStateRecord {
            plugin_id: plugin_id.to_string(),
            enabled,
            updated_at: now_rfc3339(),
        })?;
        Ok(PluginSummaryDto { enabled, ..detail.summary })
    }

    pub fn run_plugin(&self, plugin_id: &str, request: PluginExecutionRequestDto) -> Result<PluginExecutionResultDto, String> {
        let detail = self.get_plugin_detail(plugin_id)?;
        if !detail.summary.enabled {
            return Err(format!("plugin '{plugin_id}' is disabled"));
        }
        execute_built_in_plugin(self, plugin_id, request)
    }

    pub fn create_investigation_package(&self, request: CreateInvestigationPackageRequestDto) -> Result<InvestigationPackageSummaryDto, String> {
        let bundle = self.build_investigation_package(&request)?;
        write_investigation_package_bundle(&bundle)?;
        let summary = summarize_package_manifest(None, &bundle.package_path, &bundle.manifest);
        self.storage.insert_recent_package(&InsertPackageRecord {
            project_id: summary.project_id,
            created_at: summary.created_at.clone(),
            package_name: summary.package_name.clone(),
            package_path: summary.package_path.clone(),
            source_context: summary.source_context.clone(),
            schema_version: summary.schema_version,
            fwmap_version: summary.fwmap_version.clone(),
            note: summary.notes.clone(),
        })?;
        Ok(summary)
    }

    pub fn export_package(&self, request: CreateInvestigationPackageRequestDto) -> Result<InvestigationPackageSummaryDto, String> {
        self.create_investigation_package(request)
    }

    pub fn open_investigation_package(&self, path: &str) -> Result<OpenInvestigationPackageResultDto, String> {
        load_investigation_package(path)
    }

    pub fn get_investigation_package_summary(&self, path: &str) -> Result<InvestigationPackageSummaryDto, String> {
        let package = load_investigation_package(path)?;
        Ok(package.summary)
    }

    pub fn list_recent_packages(&self, project_id: Option<i64>, limit: usize) -> Result<Vec<InvestigationPackageSummaryDto>, String> {
        Ok(self
            .storage
            .list_recent_packages(project_id, limit)?
            .into_iter()
            .map(map_recent_package_summary)
            .collect())
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

    fn build_investigation_package(&self, request: &CreateInvestigationPackageRequestDto) -> Result<InvestigationPackageBundle, String> {
        if request.package_name.trim().is_empty() {
            return Err("package name must not be empty".to_string());
        }
        if request.destination_path.trim().is_empty() {
            return Err("package destination path must not be empty".to_string());
        }
        let package_path = PathBuf::from(&request.destination_path);
        let active_project = self.get_active_project()?.active_project;
        let project = match request.project_id {
            Some(project_id) => self.storage.get_project(project_id)?.map(map_project_detail),
            None => active_project,
        };
        let project_id = project.as_ref().map(|item| item.project_id);
        let project_name = project.as_ref().map(|item| item.name.clone());
        let created_at = now_rfc3339();
        let mut files: Vec<PackageFileEntry> = Vec::new();
        let mut included_items = Vec::new();
        let mut omitted_items = default_omitted_package_items();
        let mut related_run_ids = Vec::new();
        let mut related_commit_refs = Vec::new();
        let include_sections = normalize_package_sections(&request.include_sections, &request.source_context);
        let include_section = |name: &str| include_sections.iter().any(|item| item == name);

        let mut dashboard_summary = None;
        if include_section("dashboard") {
            let query = request.dashboard_query.clone().unwrap_or_else(|| DashboardQueryDto {
                repo_path: project.as_ref().and_then(|item| item.git_repo_path.clone()),
                branch: None,
                profile: project.as_ref().and_then(|item| item.default_profile.clone()),
                toolchain: None,
                target: project.as_ref().and_then(|item| item.default_target.clone()),
                limit: Some(20),
            });
            let summary = self.dashboard_summary(query)?;
            if let Some(item) = summary.latest_run.as_ref() {
                related_run_ids.push(item.run_id);
            }
            if let Some(item) = summary.latest_history_item.as_ref().and_then(|item| item.git_revision.clone()) {
                related_commit_refs.push(item);
            }
            add_package_json(&mut files, &mut included_items, "dashboard-summary.json", "dashboard", "Dashboard summary", &summary)?;
            dashboard_summary = Some(summary);
        }

        let mut run_detail = None;
        if include_section("run") {
            if let Some(run_id) = request.run_id {
                let detail = self.run_detail(run_id)?.ok_or_else(|| format!("run {run_id} was not found"))?;
                related_run_ids.push(detail.run.run_id);
                if let Some(commit) = detail.run.git_revision.clone() {
                    related_commit_refs.push(commit);
                }
                add_package_json(&mut files, &mut included_items, "run-detail.json", "run", "Run detail", &detail)?;
                run_detail = Some(detail);
            } else {
                omitted_items.push(omitted_package_item("run-detail.json", "run", "Run detail", "No run id was supplied for this package."));
            }
        }

        if include_section("diff") {
            if let Some(compare) = request.compare.clone() {
                let diff = self.compare_runs(compare.clone())?;
                related_run_ids.push(diff.left_run.run_id);
                related_run_ids.push(diff.right_run.run_id);
                if let Some(commit) = diff.left_run.git_revision.clone() {
                    related_commit_refs.push(commit);
                }
                if let Some(commit) = diff.right_run.git_revision.clone() {
                    related_commit_refs.push(commit);
                }
                add_package_json(&mut files, &mut included_items, "diff-result.json", "diff", "Diff result", &diff)?;
            } else {
                omitted_items.push(omitted_package_item("diff-result.json", "diff", "Diff result", "No compare request was supplied for this package."));
            }
        }

        let mut timeline = None;
        if include_section("history") {
            let query = request.history_query.clone().unwrap_or_else(|| HistoryQueryDto {
                repo_path: project.as_ref().and_then(|item| item.git_repo_path.clone()),
                branch: None,
                profile: project.as_ref().and_then(|item| item.default_profile.clone()),
                toolchain: None,
                target: project.as_ref().and_then(|item| item.default_target.clone()),
                limit: Some(20),
                order: Some("ancestry".to_string()),
            });
            let result = self.timeline(query)?;
            related_commit_refs.extend(result.rows.iter().take(8).map(|item| item.commit.clone()));
            add_package_json(&mut files, &mut included_items, "timeline.json", "history", "Commit timeline", &result)?;
            timeline = Some(result);
        }

        let mut range_diff = None;
        if include_section("range") {
            if let Some(query) = request.range_query.clone() {
                let result = self.get_range_diff(query)?;
                related_commit_refs.push(result.resolved_base.clone());
                related_commit_refs.push(result.resolved_head.clone());
                add_package_json(&mut files, &mut included_items, "range-diff.json", "range", "Range diff", &result)?;
                range_diff = Some(result);
            } else {
                omitted_items.push(omitted_package_item("range-diff.json", "range", "Range diff", "No range query was supplied for this package."));
            }
        }

        if include_section("regression") {
            if let Some(query) = request.regression_query.clone() {
                let result = self.detect_regression(query)?;
                if let Some(item) = result.first_observed_bad.as_ref() {
                    related_commit_refs.push(item.commit.clone());
                }
                add_package_json(&mut files, &mut included_items, "regression-result.json", "regression", "Regression result", &result)?;
            } else {
                omitted_items.push(omitted_package_item("regression-result.json", "regression", "Regression result", "No regression query was supplied for this package."));
            }
        }

        if include_section("inspector") {
            if let Some(query) = request.inspector_query.clone() {
                let summary = self.get_inspector_summary(query.clone())?;
                add_package_json(&mut files, &mut included_items, "inspector-summary.json", "inspector", "Inspector summary", &summary)?;
                if let Some(selection) = request.inspector_selection.clone() {
                    let detail = self.get_inspector_detail(query.clone(), selection.clone())?;
                    let source = self.get_source_context(query, selection)?;
                    add_package_json(&mut files, &mut included_items, "inspector-detail.json", "inspector", "Inspector detail", &detail)?;
                    add_package_json(&mut files, &mut included_items, "inspector-source-context.json", "inspector", "Inspector source context", &source)?;
                }
            } else {
                omitted_items.push(omitted_package_item("inspector-summary.json", "inspector", "Inspector summary", "No inspector query was supplied for this package."));
            }
        }

        if request.include_policy_snapshot {
            let policy = self.load_policy(project_id, None)?;
            if !policy.content.trim().is_empty() {
                add_package_json(&mut files, &mut included_items, "policy-document.json", "policy", "Policy snapshot", &policy)?;
            } else {
                omitted_items.push(omitted_package_item("policy-document.json", "policy", "Policy snapshot", "No policy file was available for the current project."));
            }
        }

        if request.include_charts_snapshot {
            let charts = serde_json::json!({
                "dashboard": dashboard_summary,
                "timeline": timeline,
                "rangeDiff": range_diff,
            });
            add_package_json(&mut files, &mut included_items, "charts-snapshot.json", "chart", "Charts snapshot", &charts)?;
        }

        let mut manifest = InvestigationPackageManifestDto {
            schema_version: INVESTIGATION_PACKAGE_SCHEMA_VERSION,
            package_version: "1.0.0".to_string(),
            package_name: request.package_name.clone(),
            created_at: created_at.clone(),
            fwmap_version: env!("CARGO_PKG_VERSION").to_string(),
            project_id,
            project_name,
            source_context: request.source_context.clone(),
            source_label: build_package_source_label(request),
            related_run_ids: unique_i64s(related_run_ids),
            related_commit_refs: unique_strings(related_commit_refs),
            git_repo_path: project.as_ref().and_then(|item| item.git_repo_path.clone()),
            git_branch: timeline.as_ref().and_then(|item| item.branch.clone()).or_else(|| run_detail.as_ref().and_then(|item| item.git_branch.clone())),
            git_revision: run_detail.as_ref().and_then(|item| item.run.git_revision.clone()).or_else(|| timeline.as_ref().and_then(|item| item.rows.first().map(|row| row.commit.clone()))),
            included_items,
            omitted_items,
            export_provenance: vec![
                ("desktop_version".to_string(), env!("CARGO_PKG_VERSION").to_string()),
                ("source_context".to_string(), request.source_context.clone()),
                ("history_db_path".to_string(), self.storage.load_settings()?.history_db_path),
            ],
            notes: request.notes.clone().filter(|value| request.include_notes && !value.trim().is_empty()),
            plugin_results: Vec::new(),
            missing_resources: Vec::new(),
        };

        if request.include_plugin_results {
            manifest.plugin_results = collect_package_plugin_results(self, request, &manifest)?;
        }

        let manifest_json = serde_json::to_vec_pretty(&manifest)
            .map_err(|err| format!("failed to serialize package manifest: {err}"))?;
        files.push(PackageFileEntry {
            relative_path: "manifest.json".to_string(),
            bytes: manifest_json,
        });

        Ok(InvestigationPackageBundle {
            package_path,
            manifest,
            files,
        })
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
        // Desktop compare is intentionally backed by persisted history metrics rather than
        // cached UI state, so the result stays reproducible after app restarts.
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

    pub fn get_inspector_summary(&self, query: InspectorQueryDto) -> Result<InspectorSummaryDto, String> {
        let context = self.resolve_inspector_context(&query)?;
        let conn = Connection::open(&context.db_path)
            .map_err(|err| format!("failed to open history database '{}': {err}", context.db_path.display()))?;
        let rows = load_inspector_rows(&conn, context.current_build_id, context.previous_build_id, &normalize_inspector_query(query.clone()))?;
        let total_size_bytes = rows.iter().map(|item| item.size_bytes).sum::<u64>();
        let total_delta_bytes = rows.iter().map(|item| item.delta_bytes).sum::<i64>();
        Ok(InspectorSummaryDto {
            context_label: context.context_label,
            source_kind: context.source_kind,
            entity_count: rows.len(),
            total_size_bytes,
            total_delta_bytes,
            debug_info_available: context.debug_info_available,
            available_views: vec![
                "region-section".to_string(),
                "source-file".to_string(),
                "function-symbol".to_string(),
                "crate-dependency".to_string(),
            ],
            available_visualizations: vec!["treemap".to_string(), "icicle".to_string(), "table".to_string()],
        })
    }

    pub fn get_inspector_breakdown(&self, query: InspectorQueryDto) -> Result<InspectorBreakdownDto, String> {
        let query = normalize_inspector_query(query);
        let context = self.resolve_inspector_context(&query)?;
        let conn = Connection::open(&context.db_path)
            .map_err(|err| format!("failed to open history database '{}': {err}", context.db_path.display()))?;
        Ok(InspectorBreakdownDto {
            query: query.clone(),
            items: load_inspector_rows(&conn, context.current_build_id, context.previous_build_id, &query)?,
        })
    }

    pub fn get_inspector_hierarchy(&self, query: InspectorQueryDto) -> Result<Vec<InspectorHierarchyNodeDto>, String> {
        let query = normalize_inspector_query(query);
        let context = self.resolve_inspector_context(&query)?;
        let conn = Connection::open(&context.db_path)
            .map_err(|err| format!("failed to open history database '{}': {err}", context.db_path.display()))?;
        let rows = load_inspector_rows(&conn, context.current_build_id, context.previous_build_id, &query)?;
        Ok(build_inspector_hierarchy(&rows, &query))
    }

    pub fn get_inspector_detail(&self, query: InspectorQueryDto, selection: InspectorSelectionDto) -> Result<InspectorDetailDto, String> {
        let query = normalize_inspector_query(query);
        let context = self.resolve_inspector_context(&query)?;
        let conn = Connection::open(&context.db_path)
            .map_err(|err| format!("failed to open history database '{}': {err}", context.db_path.display()))?;
        let rows = load_inspector_rows(&conn, context.current_build_id, context.previous_build_id, &query)?;
        let item = rows
            .into_iter()
            .find(|item| item.stable_id == selection.stable_id)
            .ok_or_else(|| format!("inspector item '{}' was not found", selection.stable_id))?;
        Ok(InspectorDetailDto {
            stable_id: item.stable_id,
            label: item.display_label.clone(),
            kind: item.kind.clone(),
            size_bytes: item.size_bytes,
            delta_bytes: item.delta_bytes,
            parent_label: item.parent_id.clone(),
            source_available: item.source_available,
            metadata: item.metadata,
            related_rule_violations: load_related_rule_hits(&conn, context.current_build_id, &item.display_label)?,
            related_regression_evidence: build_related_regression_evidence(&query, &context),
        })
    }

    pub fn get_source_context(&self, query: InspectorQueryDto, selection: InspectorSelectionDto) -> Result<SourceContextDto, String> {
        let query = normalize_inspector_query(query);
        let context = self.resolve_inspector_context(&query)?;
        let conn = Connection::open(&context.db_path)
            .map_err(|err| format!("failed to open history database '{}': {err}", context.db_path.display()))?;
        load_source_context(&conn, context.current_build_id, &selection)
    }

    fn resolve_inspector_context(&self, query: &InspectorQueryDto) -> Result<InspectorContext, String> {
        let settings = self.storage.load_settings()?;
        if let (Some(left_run_id), Some(right_run_id)) = (query.left_run_id, query.right_run_id) {
            let left = self.storage.get_recent_run(left_run_id)?.ok_or_else(|| format!("run {left_run_id} was not found"))?;
            let right = self.storage.get_recent_run(right_run_id)?.ok_or_else(|| format!("run {right_run_id} was not found"))?;
            return Ok(InspectorContext {
                db_path: PathBuf::from(&right.history_db_path),
                current_build_id: right.build_id,
                previous_build_id: Some(left.build_id),
                context_label: format!("Diff #{left_run_id} -> #{right_run_id}"),
                source_kind: "diff".to_string(),
                debug_info_available: build_has_debug_info(Path::new(&right.history_db_path), right.build_id)?,
            });
        }
        if let Some(run_id) = query.run_id {
            let current = self.storage.get_recent_run(run_id)?.ok_or_else(|| format!("run {run_id} was not found"))?;
            let previous = self.storage.list_recent_runs(50, 0)?.into_iter().find(|item| item.run_id != run_id);
            return Ok(InspectorContext {
                db_path: PathBuf::from(&current.history_db_path),
                current_build_id: current.build_id,
                previous_build_id: previous.as_ref().map(|item| item.build_id),
                context_label: format!("Run #{run_id}"),
                source_kind: "run".to_string(),
                debug_info_available: build_has_debug_info(Path::new(&settings.history_db_path), current.build_id)?,
            });
        }
        if let Some(build_id) = query.build_id {
            let history_db_path = PathBuf::from(&settings.history_db_path);
            return Ok(InspectorContext {
                db_path: history_db_path.clone(),
                current_build_id: build_id,
                previous_build_id: None,
                context_label: format!("Build #{build_id}"),
                source_kind: "history".to_string(),
                debug_info_available: build_has_debug_info(&history_db_path, build_id)?,
            });
        }
        let current = self.storage.list_recent_runs(1, 0)?.into_iter().next().ok_or_else(|| "no runs are available yet".to_string())?;
        let previous = self.storage.list_recent_runs(20, 0)?.into_iter().find(|item| item.run_id != current.run_id);
        Ok(InspectorContext {
            db_path: PathBuf::from(&settings.history_db_path),
            current_build_id: current.build_id,
            previous_build_id: previous.as_ref().map(|item| item.build_id),
            context_label: format!("Run #{}", current.run_id),
            source_kind: "run".to_string(),
            debug_info_available: build_has_debug_info(Path::new(&settings.history_db_path), current.build_id)?,
        })
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
        // Recent runs only keep lightweight metadata in the app DB; the richer detail
        // view is reconstructed on demand from history.db so storage stays normalized.
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
    // Reuse the same aggregated history tables as the CLI reports so desktop diff/history
    // views and command-line outputs stay aligned.
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
    let key_expr = resolved_history_text_expr(table, key_column);
    let sql = format!("SELECT {key_expr}, {value_column} FROM {table} WHERE build_id = ?1");
    let mut stmt = conn.prepare(&sql).map_err(|err| format!("failed to prepare metric query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|err| format!("failed to query metric rows: {err}"))?;
    let pairs = rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to read metric rows: {err}"))?;
    Ok(pairs.into_iter().collect())
}

fn load_scoped_metric_map(conn: &Connection, scope: &str, build_id: i64) -> Result<BTreeMap<String, i64>, String> {
    let name_expr = resolved_history_text_expr("rust_aggregate_metrics", "name");
    let scope_expr = resolved_history_text_expr("rust_aggregate_metrics", "scope");
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {name_expr}, size_bytes FROM rust_aggregate_metrics WHERE build_id = ?1 AND {scope_expr} = ?2"
        ))
        .map_err(|err| format!("failed to prepare rust metric query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id, scope], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|err| format!("failed to query rust metric rows: {err}"))?;
    let pairs = rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to read rust metric rows: {err}"))?;
    Ok(pairs.into_iter().collect())
}

fn load_like_scoped_metric_map(conn: &Connection, scope_like: &str, build_id: i64) -> Result<BTreeMap<String, i64>, String> {
    let name_expr = resolved_history_text_expr("rust_aggregate_metrics", "name");
    let scope_expr = resolved_history_text_expr("rust_aggregate_metrics", "scope");
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {name_expr}, size_bytes FROM rust_aggregate_metrics WHERE build_id = ?1 AND {scope_expr} LIKE ?2"
        ))
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

fn resolved_history_text_expr(table: &str, column: &str) -> String {
    // Desktop reads history.db directly in a few places, so it mirrors the core layer's
    // pooled-string resolution instead of assuming legacy plain-text columns.
    let id_column = match (table, column) {
        ("source_file_metrics", "path") => Some("path_text_id"),
        ("source_file_metrics", "display_path") => Some("display_path_text_id"),
        ("source_file_metrics", "directory") => Some("directory_text_id"),
        ("object_metrics", "object_path") => Some("object_path_text_id"),
        ("function_metrics", "function_key") => Some("function_key_text_id"),
        ("function_metrics", "raw_name") => Some("raw_name_text_id"),
        ("function_metrics", "demangled_name") => Some("demangled_name_text_id"),
        ("function_metrics", "path") => Some("path_text_id"),
        ("symbol_metrics", "name") => Some("name_text_id"),
        ("symbol_metrics", "demangled_name") => Some("demangled_name_text_id"),
        ("rule_results", "code") => Some("code_text_id"),
        ("rule_results", "level") => Some("level_text_id"),
        ("rule_results", "related") => Some("related_text_id"),
        ("rule_results", "message") => Some("message_text_id"),
        ("why_linked_metrics", "target") => Some("target_text_id"),
        ("why_linked_metrics", "kind") => Some("kind_text_id"),
        ("why_linked_metrics", "confidence") => Some("confidence_text_id"),
        ("why_linked_metrics", "summary") => Some("summary_text_id"),
        ("rust_aggregate_metrics", "scope") => Some("scope_text_id"),
        ("rust_aggregate_metrics", "name") => Some("name_text_id"),
        _ => None,
    };
    match id_column {
        Some(id_column) => format!(
            "COALESCE(NULLIF({column}, ''), (SELECT value FROM text_pool WHERE id = {id_column}))"
        ),
        None => column.to_string(),
    }
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



#[derive(Debug, Clone)]
struct InspectorContext {
    db_path: PathBuf,
    current_build_id: i64,
    previous_build_id: Option<i64>,
    context_label: String,
    source_kind: String,
    debug_info_available: bool,
}

#[derive(Debug, Clone)]
struct InspectorRowRecord {
    stable_id: String,
    display_label: String,
    raw_label: String,
    kind: String,
    parent_id: Option<String>,
    size_bytes: u64,
    source_available: bool,
    metadata: BTreeMap<String, String>,
}

fn normalize_inspector_query(mut query: InspectorQueryDto) -> InspectorQueryDto {
    if query.view_mode.trim().is_empty() {
        query.view_mode = "region-section".to_string();
    }
    if query.group_by.trim().is_empty() {
        query.group_by = match query.view_mode.as_str() {
            "source-file" => "file",
            "function-symbol" => "function",
            "crate-dependency" => "crate",
            _ => "section",
        }
        .to_string();
    }
    if query.metric.trim().is_empty() {
        query.metric = "size".to_string();
    }
    if query.top_n.is_none() {
        query.top_n = Some(24);
    }
    query
}

fn build_has_debug_info(db_path: &Path, build_id: i64) -> Result<bool, String> {
    let conn = Connection::open(db_path)
        .map_err(|err| format!("failed to open history database '{}': {err}", db_path.display()))?;
    let value = conn
        .query_row(
            "SELECT dwarf_used FROM debug_metrics WHERE build_id = ?1 LIMIT 1",
            params![build_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|err| format!("failed to query debug_metrics: {err}"))?;
    Ok(value.unwrap_or_default() != 0)
}

fn load_inspector_rows(
    conn: &Connection,
    current_build_id: i64,
    previous_build_id: Option<i64>,
    query: &InspectorQueryDto,
) -> Result<Vec<InspectorItemDto>, String> {
    let current = match query.group_by.as_str() {
        "region" => load_region_rows(conn, current_build_id)?,
        "section" => load_section_rows(conn, current_build_id)?,
        "file" => load_source_file_rows(conn, current_build_id)?,
        "directory" => load_directory_rows(conn, current_build_id)?,
        "function" => load_function_rows(conn, current_build_id)?,
        "symbol" => load_symbol_rows(conn, current_build_id)?,
        "dependency" => load_rust_scope_rows(conn, current_build_id, "dependency")?,
        "crate" => load_rust_scope_rows(conn, current_build_id, "crate")?,
        _ => load_section_rows(conn, current_build_id)?,
    };
    let previous_map = match previous_build_id {
        Some(build_id) => load_previous_map(conn, build_id, query)?,
        None => BTreeMap::new(),
    };
    let total = current.iter().map(|item| item.size_bytes).sum::<u64>().max(1);
    let mut items = current
        .into_iter()
        .map(|item| {
            let delta_bytes = item.size_bytes as i64 - previous_map.get(&item.stable_id).copied().unwrap_or_default();
            InspectorItemDto {
                stable_id: item.stable_id,
                display_label: item.display_label,
                raw_label: item.raw_label,
                kind: item.kind,
                size_bytes: item.size_bytes,
                delta_bytes,
                percentage: item.size_bytes as f64 / total as f64,
                parent_id: item.parent_id,
                has_children: false,
                source_available: item.source_available,
                metadata: item.metadata,
            }
        })
        .collect::<Vec<_>>();
    mark_hierarchy_flags(&mut items, query);
    apply_inspector_filters(&mut items, query);
    Ok(items)
}

fn load_previous_map(conn: &Connection, build_id: i64, query: &InspectorQueryDto) -> Result<BTreeMap<String, i64>, String> {
    let rows = match query.group_by.as_str() {
        "region" => load_region_rows(conn, build_id)?,
        "section" => load_section_rows(conn, build_id)?,
        "file" => load_source_file_rows(conn, build_id)?,
        "directory" => load_directory_rows(conn, build_id)?,
        "function" => load_function_rows(conn, build_id)?,
        "symbol" => load_symbol_rows(conn, build_id)?,
        "dependency" => load_rust_scope_rows(conn, build_id, "dependency")?,
        "crate" => load_rust_scope_rows(conn, build_id, "crate")?,
        _ => load_section_rows(conn, build_id)?,
    };
    Ok(rows.into_iter().map(|item| (item.stable_id, item.size_bytes as i64)).collect())
}

fn load_region_rows(conn: &Connection, build_id: i64) -> Result<Vec<InspectorRowRecord>, String> {
    let mut stmt = conn
        .prepare("SELECT region_name, used_bytes, free_bytes, usage_ratio FROM region_metrics WHERE build_id = ?1 ORDER BY used_bytes DESC, region_name ASC")
        .map_err(|err| format!("failed to prepare region inspector query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| {
            let name = row.get::<_, String>(0)?;
            let mut metadata = BTreeMap::new();
            metadata.insert("freeBytes".to_string(), row.get::<_, i64>(2)?.to_string());
            metadata.insert("usageRatio".to_string(), format!("{:.3}", row.get::<_, f64>(3)?));
            Ok(InspectorRowRecord {
                stable_id: format!("region:{name}"),
                display_label: name.clone(),
                raw_label: name.clone(),
                kind: "region".to_string(),
                parent_id: None,
                size_bytes: row.get::<_, i64>(1)? as u64,
                source_available: false,
                metadata,
            })
        })
        .map_err(|err| format!("failed to query region inspector rows: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect region inspector rows: {err}"))
}

fn load_section_rows(conn: &Connection, build_id: i64) -> Result<Vec<InspectorRowRecord>, String> {
    let mut stmt = conn
        .prepare("SELECT section_name, size_bytes, category FROM section_metrics WHERE build_id = ?1 ORDER BY size_bytes DESC, section_name ASC")
        .map_err(|err| format!("failed to prepare section inspector query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| {
            let name = row.get::<_, String>(0)?;
            let category = row.get::<_, String>(2)?;
            let mut metadata = BTreeMap::new();
            metadata.insert("category".to_string(), category.clone());
            Ok(InspectorRowRecord {
                stable_id: format!("section:{name}"),
                display_label: name.clone(),
                raw_label: name.clone(),
                kind: "section".to_string(),
                parent_id: Some(format!("region:{category}")),
                size_bytes: row.get::<_, i64>(1)? as u64,
                source_available: false,
                metadata,
            })
        })
        .map_err(|err| format!("failed to query section inspector rows: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect section inspector rows: {err}"))
}

fn load_source_file_rows(conn: &Connection, build_id: i64) -> Result<Vec<InspectorRowRecord>, String> {
    let mut stmt = conn
        .prepare("SELECT path, display_path, directory, size_bytes, function_count, line_range_count FROM source_file_metrics WHERE build_id = ?1 ORDER BY size_bytes DESC, display_path ASC")
        .map_err(|err| format!("failed to prepare source file inspector query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| {
            let path = row.get::<_, String>(0)?;
            let display_path = row.get::<_, String>(1)?;
            let directory = row.get::<_, String>(2)?;
            let mut metadata = BTreeMap::new();
            metadata.insert("path".to_string(), path.clone());
            metadata.insert("directory".to_string(), directory.clone());
            metadata.insert("functionCount".to_string(), row.get::<_, i64>(4)?.to_string());
            metadata.insert("lineRangeCount".to_string(), row.get::<_, i64>(5)?.to_string());
            Ok(InspectorRowRecord {
                stable_id: format!("file:{display_path}"),
                display_label: display_path.clone(),
                raw_label: path.clone(),
                kind: "file".to_string(),
                parent_id: Some(format!("directory:{directory}")),
                size_bytes: row.get::<_, i64>(3)? as u64,
                source_available: true,
                metadata,
            })
        })
        .map_err(|err| format!("failed to query source file inspector rows: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect source file inspector rows: {err}"))
}

fn load_directory_rows(conn: &Connection, build_id: i64) -> Result<Vec<InspectorRowRecord>, String> {
    let mut stmt = conn
        .prepare("SELECT directory, SUM(size_bytes), SUM(function_count), SUM(line_range_count) FROM source_file_metrics WHERE build_id = ?1 GROUP BY directory ORDER BY SUM(size_bytes) DESC, directory ASC")
        .map_err(|err| format!("failed to prepare directory inspector query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| {
            let directory = row.get::<_, String>(0)?;
            let mut metadata = BTreeMap::new();
            metadata.insert("functionCount".to_string(), row.get::<_, i64>(2)?.to_string());
            metadata.insert("lineRangeCount".to_string(), row.get::<_, i64>(3)?.to_string());
            Ok(InspectorRowRecord {
                stable_id: format!("directory:{directory}"),
                display_label: directory.clone(),
                raw_label: directory.clone(),
                kind: "directory".to_string(),
                parent_id: None,
                size_bytes: row.get::<_, i64>(1)? as u64,
                source_available: true,
                metadata,
            })
        })
        .map_err(|err| format!("failed to query directory inspector rows: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect directory inspector rows: {err}"))
}

fn load_function_rows(conn: &Connection, build_id: i64) -> Result<Vec<InspectorRowRecord>, String> {
    let mut stmt = conn
        .prepare("SELECT function_key, raw_name, COALESCE(demangled_name, raw_name), COALESCE(path, '-'), size_bytes FROM function_metrics WHERE build_id = ?1 ORDER BY size_bytes DESC, raw_name ASC")
        .map_err(|err| format!("failed to prepare function inspector query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| {
            let key = row.get::<_, String>(0)?;
            let raw_name = row.get::<_, String>(1)?;
            let display = row.get::<_, String>(2)?;
            let path_value = row.get::<_, String>(3)?;
            let mut metadata = BTreeMap::new();
            metadata.insert("path".to_string(), path_value.clone());
            metadata.insert("rawName".to_string(), raw_name.clone());
            Ok(InspectorRowRecord {
                stable_id: format!("function:{key}"),
                display_label: display.clone(),
                raw_label: raw_name,
                kind: "function".to_string(),
                parent_id: Some(format!("file:{path_value}")),
                size_bytes: row.get::<_, i64>(4)? as u64,
                source_available: path_value != "-",
                metadata,
            })
        })
        .map_err(|err| format!("failed to query function inspector rows: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect function inspector rows: {err}"))
}

fn load_symbol_rows(conn: &Connection, build_id: i64) -> Result<Vec<InspectorRowRecord>, String> {
    let mut stmt = conn
        .prepare("SELECT name, COALESCE(demangled_name, name), size_bytes FROM symbol_metrics WHERE build_id = ?1 ORDER BY size_bytes DESC, name ASC")
        .map_err(|err| format!("failed to prepare symbol inspector query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id], |row| {
            let raw = row.get::<_, String>(0)?;
            let display = row.get::<_, String>(1)?;
            let mut metadata = BTreeMap::new();
            metadata.insert("rawName".to_string(), raw.clone());
            Ok(InspectorRowRecord {
                stable_id: format!("symbol:{raw}"),
                display_label: display,
                raw_label: raw,
                kind: "symbol".to_string(),
                parent_id: None,
                size_bytes: row.get::<_, i64>(2)? as u64,
                source_available: false,
                metadata,
            })
        })
        .map_err(|err| format!("failed to query symbol inspector rows: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect symbol inspector rows: {err}"))
}

fn load_rust_scope_rows(conn: &Connection, build_id: i64, scope: &str) -> Result<Vec<InspectorRowRecord>, String> {
    let mut stmt = conn
        .prepare("SELECT name, size_bytes, symbol_count FROM rust_aggregate_metrics WHERE build_id = ?1 AND scope = ?2 ORDER BY size_bytes DESC, name ASC")
        .map_err(|err| format!("failed to prepare rust inspector query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id, scope], |row| {
            let name = row.get::<_, String>(0)?;
            let mut metadata = BTreeMap::new();
            metadata.insert("symbolCount".to_string(), row.get::<_, i64>(2)?.to_string());
            Ok(InspectorRowRecord {
                stable_id: format!("{scope}:{name}"),
                display_label: name.clone(),
                raw_label: name.clone(),
                kind: scope.to_string(),
                parent_id: None,
                size_bytes: row.get::<_, i64>(1)? as u64,
                source_available: scope == "crate",
                metadata,
            })
        })
        .map_err(|err| format!("failed to query rust inspector rows: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect rust inspector rows: {err}"))
}

fn mark_hierarchy_flags(items: &mut [InspectorItemDto], query: &InspectorQueryDto) {
    let has_children = items.iter().filter_map(|item| item.parent_id.clone()).collect::<Vec<_>>();
    for item in items.iter_mut() {
        item.has_children = has_children.iter().any(|parent| parent == &item.stable_id)
            || matches!(query.view_mode.as_str(), "region-section") && item.kind == "region"
            || matches!(query.view_mode.as_str(), "source-file") && item.kind == "directory";
    }
}

fn apply_inspector_filters(items: &mut Vec<InspectorItemDto>, query: &InspectorQueryDto) {
    if let Some(search) = query.search.as_ref().map(|value| value.to_ascii_lowercase()).filter(|value| !value.is_empty()) {
        items.retain(|item| item.display_label.to_ascii_lowercase().contains(&search) || item.raw_label.to_ascii_lowercase().contains(&search));
    }
    if let Some(threshold_min) = query.threshold_min {
        items.retain(|item| item.size_bytes as i64 >= threshold_min || item.delta_bytes.unsigned_abs() as i64 >= threshold_min);
    }
    if query.only_increased.unwrap_or(false) {
        items.retain(|item| item.delta_bytes > 0);
    }
    if query.only_decreased.unwrap_or(false) {
        items.retain(|item| item.delta_bytes < 0);
    }
    if query.debug_info_only.unwrap_or(false) {
        items.retain(|item| item.source_available);
    }
    if query.metric == "delta" {
        items.sort_by(|a, b| b.delta_bytes.abs().cmp(&a.delta_bytes.abs()).then_with(|| a.display_label.cmp(&b.display_label)));
    } else {
        items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then_with(|| a.display_label.cmp(&b.display_label)));
    }
    items.truncate(query.top_n.unwrap_or(24));
}

fn build_inspector_hierarchy(items: &[InspectorItemDto], query: &InspectorQueryDto) -> Vec<InspectorHierarchyNodeDto> {
    let mut grouped = BTreeMap::<String, Vec<&InspectorItemDto>>::new();
    for item in items {
        if let Some(parent) = item.parent_id.as_ref() {
            grouped.entry(parent.clone()).or_default().push(item);
        }
    }
    let mut roots = Vec::new();
    if query.view_mode == "region-section" {
        let mut section_totals = BTreeMap::<String, (u64, i64)>::new();
        for item in items.iter().filter(|item| item.kind == "section") {
            let parent = item.parent_id.clone().unwrap_or_else(|| "region:other".to_string());
            let entry = section_totals.entry(parent).or_insert((0, 0));
            entry.0 += item.size_bytes;
            entry.1 += item.delta_bytes;
        }
        for (stable_id, (size_bytes, delta_bytes)) in section_totals {
            let label = stable_id.split_once(':').map(|(_, name)| name.to_string()).unwrap_or_else(|| stable_id.clone());
            let children = grouped.get(&stable_id).cloned().unwrap_or_default();
            roots.push(InspectorHierarchyNodeDto {
                stable_id: stable_id.clone(),
                label,
                kind: "region".to_string(),
                size_bytes,
                delta_bytes,
                percentage: 0.0,
                source_available: false,
                children: children.into_iter().map(map_hierarchy_leaf).collect(),
            });
        }
    } else if query.view_mode == "source-file" {
        let mut directory_totals = BTreeMap::<String, (u64, i64)>::new();
        for item in items.iter().filter(|item| item.kind == "file") {
            let parent = item.parent_id.clone().unwrap_or_else(|| "directory:-".to_string());
            let entry = directory_totals.entry(parent).or_insert((0, 0));
            entry.0 += item.size_bytes;
            entry.1 += item.delta_bytes;
        }
        for (stable_id, (size_bytes, delta_bytes)) in directory_totals {
            let label = stable_id.split_once(':').map(|(_, name)| name.to_string()).unwrap_or_else(|| stable_id.clone());
            let children = grouped.get(&stable_id).cloned().unwrap_or_default();
            roots.push(InspectorHierarchyNodeDto {
                stable_id: stable_id.clone(),
                label,
                kind: "directory".to_string(),
                size_bytes,
                delta_bytes,
                percentage: 0.0,
                source_available: true,
                children: children.into_iter().map(map_hierarchy_leaf).collect(),
            });
        }
    } else {
        roots = items.iter().map(map_hierarchy_leaf).collect();
    }
    let total = roots.iter().map(|item| item.size_bytes).sum::<u64>().max(1);
    for item in &mut roots {
        item.percentage = item.size_bytes as f64 / total as f64;
        let child_total = item.children.iter().map(|child| child.size_bytes).sum::<u64>().max(1);
        for child in &mut item.children {
            child.percentage = child.size_bytes as f64 / child_total as f64;
        }
    }
    roots.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then_with(|| a.label.cmp(&b.label)));
    roots.truncate(query.top_n.unwrap_or(24).min(16));
    roots
}

fn map_hierarchy_leaf(item: &InspectorItemDto) -> InspectorHierarchyNodeDto {
    InspectorHierarchyNodeDto {
        stable_id: item.stable_id.clone(),
        label: item.display_label.clone(),
        kind: item.kind.clone(),
        size_bytes: item.size_bytes,
        delta_bytes: item.delta_bytes,
        percentage: item.percentage,
        source_available: item.source_available,
        children: Vec::new(),
    }
}

fn load_related_rule_hits(conn: &Connection, build_id: i64, label: &str) -> Result<Vec<String>, String> {
    let like = format!("%{label}%");
    let mut stmt = conn
        .prepare("SELECT code, level, message FROM rule_results WHERE build_id = ?1 AND (related LIKE ?2 OR message LIKE ?2) ORDER BY id ASC LIMIT 8")
        .map_err(|err| format!("failed to prepare related rule query: {err}"))?;
    let rows = stmt
        .query_map(params![build_id, like], |row| Ok(format!("{} / {} / {}", row.get::<_, String>(1)?, row.get::<_, String>(0)?, row.get::<_, String>(2)?)))
        .map_err(|err| format!("failed to query related rule hits: {err}"))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect related rule hits: {err}"))
}

fn build_related_regression_evidence(query: &InspectorQueryDto, context: &InspectorContext) -> Vec<String> {
    let mut items = vec![format!("Context: {}", context.context_label)];
    if let (Some(left), Some(right)) = (query.left_run_id, query.right_run_id) {
        items.push(format!("Derived from run diff #{left} -> #{right}"));
    }
    if let Some(run_id) = query.run_id {
        items.push(format!("Derived from run #{run_id}"));
    }
    items
}

fn load_source_context(conn: &Connection, build_id: i64, selection: &InspectorSelectionDto) -> Result<SourceContextDto, String> {
    match selection.kind.as_str() {
        "file" | "directory" => load_file_source_context(conn, build_id, selection),
        "function" => load_function_source_context(conn, build_id, selection),
        "crate" => Ok(SourceContextDto {
            path: None,
            function_name: None,
            line_start: None,
            line_end: None,
            excerpt: None,
            compile_unit: None,
            crate_name: selection.stable_id.split_once(':').map(|(_, value)| value.to_string()),
            related_sections: Vec::new(),
            related_regions: Vec::new(),
            availability_reason: Some("crate-level source context is not available in Phase D5 desktop yet".to_string()),
        }),
        _ => Ok(SourceContextDto {
            path: None,
            function_name: None,
            line_start: None,
            line_end: None,
            excerpt: None,
            compile_unit: None,
            crate_name: None,
            related_sections: Vec::new(),
            related_regions: Vec::new(),
            availability_reason: Some("source context is unavailable for this item kind".to_string()),
        }),
    }
}

fn load_file_source_context(conn: &Connection, build_id: i64, selection: &InspectorSelectionDto) -> Result<SourceContextDto, String> {
    if selection.kind == "directory" {
        return Ok(SourceContextDto {
            path: selection.stable_id.split_once(':').map(|(_, value)| value.to_string()),
            function_name: None,
            line_start: None,
            line_end: None,
            excerpt: None,
            compile_unit: selection.stable_id.split_once(':').map(|(_, value)| value.to_string()),
            crate_name: None,
            related_sections: Vec::new(),
            related_regions: Vec::new(),
            availability_reason: Some("directory view summarizes source files; select an individual file for more detail".to_string()),
        });
    }
    let path_value = selection.stable_id.split_once(':').map(|(_, value)| value.to_string()).unwrap_or_else(|| selection.stable_id.clone());
    let mut stmt = conn
        .prepare("SELECT path, directory, function_count, line_range_count FROM source_file_metrics WHERE build_id = ?1 AND display_path = ?2 LIMIT 1")
        .map_err(|err| format!("failed to prepare file source context query: {err}"))?;
    let row = stmt
        .query_row(params![build_id, path_value], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .optional()
        .map_err(|err| format!("failed to query file source context: {err}"))?;
    match row {
        Some((path, directory, function_count, line_range_count)) => Ok(SourceContextDto {
            path: Some(path),
            function_name: None,
            line_start: None,
            line_end: None,
            excerpt: Some(format!("Source file attributed in DWARF. Functions: {function_count}, line ranges: {line_range_count}.")),
            compile_unit: Some(directory),
            crate_name: None,
            related_sections: Vec::new(),
            related_regions: Vec::new(),
            availability_reason: Some("line excerpts are not embedded; this is a metadata summary".to_string()),
        }),
        None => Ok(SourceContextDto {
            path: None,
            function_name: None,
            line_start: None,
            line_end: None,
            excerpt: None,
            compile_unit: None,
            crate_name: None,
            related_sections: Vec::new(),
            related_regions: Vec::new(),
            availability_reason: Some("source file detail was not found for the selected item".to_string()),
        }),
    }
}

fn load_function_source_context(conn: &Connection, build_id: i64, selection: &InspectorSelectionDto) -> Result<SourceContextDto, String> {
    let key = selection.stable_id.split_once(':').map(|(_, value)| value.to_string()).unwrap_or_else(|| selection.stable_id.clone());
    let mut stmt = conn
        .prepare("SELECT COALESCE(path, '-'), COALESCE(demangled_name, raw_name), raw_name, size_bytes FROM function_metrics WHERE build_id = ?1 AND function_key = ?2 LIMIT 1")
        .map_err(|err| format!("failed to prepare function source context query: {err}"))?;
    let row = stmt
        .query_row(params![build_id, key], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .optional()
        .map_err(|err| format!("failed to query function source context: {err}"))?;
    match row {
        Some((path, function_name, raw_name, size_bytes)) => Ok(SourceContextDto {
            path: (path != "-").then_some(path),
            function_name: Some(function_name),
            line_start: None,
            line_end: None,
            excerpt: Some(format!("Function '{raw_name}' accounts for {} bytes in this build.", size_bytes)),
            compile_unit: None,
            crate_name: None,
            related_sections: Vec::new(),
            related_regions: Vec::new(),
            availability_reason: Some("exact source lines are unavailable in the current desktop inspector".to_string()),
        }),
        None => Ok(SourceContextDto {
            path: None,
            function_name: None,
            line_start: None,
            line_end: None,
            excerpt: None,
            compile_unit: None,
            crate_name: None,
            related_sections: Vec::new(),
            related_regions: Vec::new(),
            availability_reason: Some("function detail was not found for the selected item".to_string()),
        }),
    }
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

fn signed(value: i64) -> String {
    format!("{:+}", value)
}

fn signed_or_dash(value: Option<i64>) -> String {
    value.map(signed).unwrap_or_else(|| "-".to_string())
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

#[derive(Debug, Clone)]
struct PackageFileEntry {
    relative_path: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct InvestigationPackageBundle {
    package_path: PathBuf,
    manifest: InvestigationPackageManifestDto,
    files: Vec<PackageFileEntry>,
}

#[derive(Debug, Clone)]
struct BuiltInPluginDefinition {
    plugin_id: &'static str,
    display_name: &'static str,
    version: &'static str,
    description: &'static str,
    layer: &'static str,
    capabilities: &'static [(&'static str, &'static str, &'static str)],
    extension_points: &'static [&'static str],
    supported_input_kinds: &'static [&'static str],
    output_kinds: &'static [&'static str],
    safety_level: &'static str,
    stability_level: &'static str,
    enabled_by_default: bool,
    execution_model: &'static str,
    failure_behavior: &'static str,
    notes: &'static [&'static str],
}

const BUILT_IN_PLUGINS: &[BuiltInPluginDefinition] = &[
    BuiltInPluginDefinition {
        plugin_id: "size-posture-analyzer",
        display_name: "Size Posture Analyzer",
        version: "1.0.0",
        description: "Adds supplementary size posture summaries for run, diff, and dashboard contexts.",
        layer: "app-service",
        capabilities: &[
            ("analyzer.summary", "Analyzer summary", "Summarizes the current size posture without altering core analysis results."),
            ("investigation.annotation", "Investigation annotation", "Produces short annotations that can be embedded into packages."),
        ],
        extension_points: &["analyzer.summary"],
        supported_input_kinds: &["dashboard", "run", "diff", "history", "regression"],
        output_kinds: &["summary-card", "annotation"],
        safety_level: "internal-only",
        stability_level: "preview",
        enabled_by_default: true,
        execution_model: "Synchronous, in-process, built-in execution against existing desktop service queries.",
        failure_behavior: "Plugin failures are returned as supplementary errors and do not replace core results.",
        notes: &[
            "Uses the same desktop service APIs as the visible screens.",
            "Never mutates recorded history or analysis outputs.",
        ],
    },
    BuiltInPluginDefinition {
        plugin_id: "package-provenance-exporter",
        display_name: "Package Provenance Exporter",
        version: "1.0.0",
        description: "Contributes package-focused metadata and inclusion summaries for shareable investigation bundles.",
        layer: "app-service",
        capabilities: &[
            ("report.package", "Package report", "Adds package provenance and inclusion notes for exported investigation bundles."),
            ("export.metadata", "Export metadata", "Attaches explicit included/omitted resource summaries."),
        ],
        extension_points: &["report.package-section"],
        supported_input_kinds: &["package"],
        output_kinds: &["package-note", "provenance"],
        safety_level: "internal-only",
        stability_level: "preview",
        enabled_by_default: true,
        execution_model: "Synchronous manifest augmentation during package creation or package inspection.",
        failure_behavior: "Package creation continues even if this plugin cannot emit extra notes.",
        notes: &[
            "Designed to keep package/export responsibilities explicit.",
            "Does not bundle arbitrary external files automatically.",
        ],
    },
    BuiltInPluginDefinition {
        plugin_id: "timeline-signal-adapter",
        display_name: "Timeline Signal Adapter",
        version: "1.0.0",
        description: "Provides visualization-ready signal summaries from timeline and range contexts.",
        layer: "ui-adapter",
        capabilities: &[
            ("visualization.adapter", "Visualization adapter", "Shapes history results into concise signal blocks for dashboard or package viewers."),
        ],
        extension_points: &["visualization.adapter"],
        supported_input_kinds: &["dashboard", "history", "range", "package"],
        output_kinds: &["signal-list"],
        safety_level: "internal-only",
        stability_level: "experimental",
        enabled_by_default: true,
        execution_model: "Synchronous adapter over existing timeline/range summaries.",
        failure_behavior: "If unavailable, the UI falls back to built-in cards and tables.",
        notes: &[
            "This is the first visualization adapter extension point, not a general chart plugin host.",
        ],
    },
];

fn built_in_extension_points() -> Vec<ExtensionPointDto> {
    vec![
        ExtensionPointDto {
            extension_point_id: "analyzer.summary".to_string(),
            display_name: "Analyzer Summary".to_string(),
            description: "Read-only supplementary summaries derived from runs, diffs, or regressions.".to_string(),
            layer: "app-service".to_string(),
            supported_contexts: vec!["dashboard".to_string(), "run".to_string(), "diff".to_string(), "history".to_string(), "regression".to_string()],
        },
        ExtensionPointDto {
            extension_point_id: "report.package-section".to_string(),
            display_name: "Package Report Section".to_string(),
            description: "Adds supplementary manifest notes and package inclusion summaries during export.".to_string(),
            layer: "app-service".to_string(),
            supported_contexts: vec!["package".to_string(), "export".to_string()],
        },
        ExtensionPointDto {
            extension_point_id: "visualization.adapter".to_string(),
            display_name: "Visualization Adapter".to_string(),
            description: "Produces view-model summaries for dashboard, timeline, and package viewers.".to_string(),
            layer: "ui-adapter".to_string(),
            supported_contexts: vec!["dashboard".to_string(), "history".to_string(), "range".to_string(), "package".to_string()],
        },
    ]
}

fn built_in_plugin_summaries(states: &[crate::storage::StoredPluginStateRecord]) -> Vec<PluginSummaryDto> {
    BUILT_IN_PLUGINS
        .iter()
        .map(|definition| {
            let enabled = states
                .iter()
                .find(|item| item.plugin_id == definition.plugin_id)
                .map(|item| item.enabled)
                .unwrap_or(definition.enabled_by_default);
            map_plugin_summary(definition, enabled)
        })
        .collect()
}

fn plugin_detail(plugin_id: &str, state: Option<&crate::storage::StoredPluginStateRecord>) -> Option<PluginDetailDto> {
    let definition = BUILT_IN_PLUGINS.iter().find(|item| item.plugin_id == plugin_id)?;
    let enabled = state.map(|item| item.enabled).unwrap_or(definition.enabled_by_default);
    Some(PluginDetailDto {
        summary: map_plugin_summary(definition, enabled),
        execution_model: definition.execution_model.to_string(),
        failure_behavior: definition.failure_behavior.to_string(),
        notes: definition.notes.iter().map(|item| (*item).to_string()).collect(),
    })
}

fn map_plugin_summary(definition: &BuiltInPluginDefinition, enabled: bool) -> PluginSummaryDto {
    PluginSummaryDto {
        plugin_id: definition.plugin_id.to_string(),
        display_name: definition.display_name.to_string(),
        version: definition.version.to_string(),
        description: definition.description.to_string(),
        layer: definition.layer.to_string(),
        capabilities: definition
            .capabilities
            .iter()
            .map(|(capability_id, label, description)| PluginCapabilityDto {
                capability_id: (*capability_id).to_string(),
                label: (*label).to_string(),
                description: (*description).to_string(),
            })
            .collect(),
        extension_points: definition.extension_points.iter().map(|item| (*item).to_string()).collect(),
        supported_input_kinds: definition.supported_input_kinds.iter().map(|item| (*item).to_string()).collect(),
        output_kinds: definition.output_kinds.iter().map(|item| (*item).to_string()).collect(),
        safety_level: definition.safety_level.to_string(),
        stability_level: definition.stability_level.to_string(),
        enabled,
        status: if enabled { "ready".to_string() } else { "disabled".to_string() },
        last_error: None,
    }
}

fn execute_built_in_plugin(state: &DesktopState, plugin_id: &str, request: PluginExecutionRequestDto) -> Result<PluginExecutionResultDto, String> {
    match plugin_id {
        "size-posture-analyzer" => execute_size_posture_plugin(state, request),
        "package-provenance-exporter" => execute_package_provenance_plugin(request),
        "timeline-signal-adapter" => execute_timeline_signal_plugin(state, request),
        _ => Err(format!("plugin '{plugin_id}' was not found")),
    }
}

fn execute_size_posture_plugin(state: &DesktopState, request: PluginExecutionRequestDto) -> Result<PluginExecutionResultDto, String> {
    let mut output_items = Vec::new();
    let summary = match request.context_kind.as_str() {
        "run" => {
            let run_id = request.run_id.ok_or_else(|| "run plugin execution requires run_id".to_string())?;
            let detail = state.run_detail(run_id)?.ok_or_else(|| format!("run {run_id} was not found"))?;
            output_items.push(PluginOutputItemDto {
                kind: "size".to_string(),
                title: "Footprint".to_string(),
                summary: format!("ROM {} / RAM {}", format_bytes_compact(detail.run.rom_bytes), format_bytes_compact(detail.run.ram_bytes)),
                detail: Some(format!("{} warnings, top section {}", detail.warnings.len(), detail.top_sections.first().map(|item| item.0.as_str()).unwrap_or("-"))),
            });
            format!("Run #{} is carrying {} ROM and {} RAM.", detail.run.run_id, format_bytes_compact(detail.run.rom_bytes), format_bytes_compact(detail.run.ram_bytes))
        }
        "diff" => {
            let left = request.left_run_id.ok_or_else(|| "diff plugin execution requires left_run_id".to_string())?;
            let right = request.right_run_id.ok_or_else(|| "diff plugin execution requires right_run_id".to_string())?;
            let diff = state.compare_runs(RunCompareRequestDto { left_run_id: left, right_run_id: right })?;
            output_items.push(PluginOutputItemDto {
                kind: "delta".to_string(),
                title: "ROM / RAM delta".to_string(),
                summary: format!("ROM {} / RAM {}", signed(diff.summary.rom_delta), signed(diff.summary.ram_delta)),
                detail: Some(format!("Warning delta {}", signed(diff.summary.warning_delta))),
            });
            format!("Diff #{} -> #{} changed ROM by {} and RAM by {}.", left, right, signed(diff.summary.rom_delta), signed(diff.summary.ram_delta))
        }
        _ => {
            let history_query = request.history_query.unwrap_or_default();
            let summary = state.dashboard_summary(DashboardQueryDto {
                repo_path: history_query.repo_path,
                branch: history_query.branch,
                profile: history_query.profile,
                toolchain: history_query.toolchain,
                target: history_query.target,
                limit: history_query.limit,
            })?;
            output_items.push(PluginOutputItemDto {
                kind: "signal".to_string(),
                title: "Recent regressions".to_string(),
                summary: format!("{} recent regression signals", summary.recent_regressions.len()),
                detail: summary.recent_regressions.first().map(|item| item.reasoning.clone()),
            });
            format!("Dashboard currently exposes {} recent regression signals and {} overview cards.", summary.recent_regressions.len(), summary.overview_cards.len())
        }
    };
    Ok(PluginExecutionResultDto {
        plugin_id: "size-posture-analyzer".to_string(),
        status: "ok".to_string(),
        summary,
        warnings: Vec::new(),
        output_items,
    })
}

fn execute_package_provenance_plugin(request: PluginExecutionRequestDto) -> Result<PluginExecutionResultDto, String> {
    let package_path = request.package_path.ok_or_else(|| "package provenance plugin requires package_path".to_string())?;
    let opened = load_investigation_package(&package_path)?;
    Ok(PluginExecutionResultDto {
        plugin_id: "package-provenance-exporter".to_string(),
        status: "ok".to_string(),
        summary: format!("Package '{}' includes {} items and omits {} items.", opened.summary.package_name, opened.summary.included_count, opened.summary.omitted_count),
        warnings: opened.summary.warnings.clone(),
        output_items: vec![
            PluginOutputItemDto {
                kind: "provenance".to_string(),
                title: "Source context".to_string(),
                summary: opened.manifest.source_context.clone(),
                detail: Some(opened.manifest.source_label.clone()),
            },
            PluginOutputItemDto {
                kind: "coverage".to_string(),
                title: "Included resources".to_string(),
                summary: format!("{} included / {} omitted", opened.summary.included_count, opened.summary.omitted_count),
                detail: Some(opened.manifest.missing_resources.join(", ")),
            },
        ],
    })
}

fn execute_timeline_signal_plugin(state: &DesktopState, request: PluginExecutionRequestDto) -> Result<PluginExecutionResultDto, String> {
    let result = if let Some(query) = request.range_query {
        let range = state.get_range_diff(query)?;
        PluginExecutionResultDto {
            plugin_id: "timeline-signal-adapter".to_string(),
            status: "ok".to_string(),
            summary: format!("Range {} -> {} spans {} analyzed commits.", range.resolved_base, range.resolved_head, range.analyzed_commits_count),
            warnings: Vec::new(),
            output_items: vec![
                PluginOutputItemDto {
                    kind: "signal".to_string(),
                    title: "Largest ROM movement".to_string(),
                    summary: range.worst_commit_by_rom.as_ref().map(|item| format!("{} ({})", item.commit, signed(item.delta))).unwrap_or_else(|| "No ROM signal".to_string()),
                    detail: range.worst_commit_by_rom.as_ref().map(|item| item.subject.clone()),
                },
            ],
        }
    } else if let Some(history_query) = request.history_query {
        let timeline = state.timeline(history_query)?;
        PluginExecutionResultDto {
            plugin_id: "timeline-signal-adapter".to_string(),
            status: "ok".to_string(),
            summary: format!("Timeline has {} visible rows.", timeline.rows.len()),
            warnings: Vec::new(),
            output_items: timeline.rows.iter().take(3).map(|row| PluginOutputItemDto {
                kind: "commit".to_string(),
                title: row.short_commit.clone(),
                summary: row.subject.clone(),
                detail: Some(format!("ROM {} / delta {}", format_bytes_compact(row.rom_total), signed_or_dash(row.rom_delta_vs_previous))),
            }).collect(),
        }
    } else {
        return Err("timeline signal adapter requires history_query or range_query".to_string());
    };
    Ok(result)
}

fn normalize_package_sections(include_sections: &[String], source_context: &str) -> Vec<String> {
    let mut sections = if include_sections.is_empty() {
        vec![source_context.to_string()]
    } else {
        include_sections.to_vec()
    };
    if !sections.iter().any(|item| item == source_context) {
        sections.push(source_context.to_string());
    }
    if !sections.iter().any(|item| item == "dashboard") {
        sections.push("dashboard".to_string());
    }
    unique_strings(sections)
}

fn add_package_json<T: serde::Serialize>(
    files: &mut Vec<PackageFileEntry>,
    included_items: &mut Vec<InvestigationPackageItemDto>,
    relative_path: &str,
    kind: &str,
    title: &str,
    value: &T,
) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|err| format!("failed to serialize package item '{relative_path}': {err}"))?;
    files.push(PackageFileEntry { relative_path: relative_path.to_string(), bytes });
    included_items.push(InvestigationPackageItemDto {
        relative_path: relative_path.to_string(),
        kind: kind.to_string(),
        title: title.to_string(),
        included: true,
        description: format!("Included {} content.", kind),
        missing_reason: None,
    });
    Ok(())
}

fn default_omitted_package_items() -> Vec<InvestigationPackageItemDto> {
    vec![
        omitted_package_item("source-tree", "resource", "Source tree", "Source files are referenced by metadata only and are not bundled automatically."),
        omitted_package_item("binary-artifact", "resource", "Binary artifact", "The full ELF/MAP artifacts are omitted unless exported separately."),
        omitted_package_item("history-db", "resource", "History database", "The live history database is not copied into the package bundle."),
    ]
}

fn omitted_package_item(relative_path: &str, kind: &str, title: &str, reason: &str) -> InvestigationPackageItemDto {
    InvestigationPackageItemDto {
        relative_path: relative_path.to_string(),
        kind: kind.to_string(),
        title: title.to_string(),
        included: false,
        description: title.to_string(),
        missing_reason: Some(reason.to_string()),
    }
}

fn build_package_source_label(request: &CreateInvestigationPackageRequestDto) -> String {
    match request.source_context.as_str() {
        "run" => request.run_id.map(|item| format!("Run #{item}")),
        "diff" => request.compare.as_ref().map(|item| format!("Diff #{} -> #{}", item.left_run_id, item.right_run_id)),
        "range" => request.range_query.as_ref().map(|item| item.spec.clone()),
        "regression" => request.regression_query.as_ref().map(|item| format!("{} / {}", item.spec, item.key)),
        "inspector" => request.inspector_selection.as_ref().map(|item| item.stable_id.clone()),
        _ => Some("Dashboard snapshot".to_string()),
    }
    .unwrap_or_else(|| request.source_context.clone())
}

fn unique_strings(values: Vec<String>) -> Vec<String> {
    let mut ordered = Vec::new();
    for value in values {
        if !value.trim().is_empty() && !ordered.iter().any(|item| item == &value) {
            ordered.push(value);
        }
    }
    ordered
}

fn unique_i64s(values: Vec<i64>) -> Vec<i64> {
    let mut ordered = Vec::new();
    for value in values {
        if !ordered.iter().any(|item| item == &value) {
            ordered.push(value);
        }
    }
    ordered
}

fn collect_package_plugin_results(
    state: &DesktopState,
    request: &CreateInvestigationPackageRequestDto,
    manifest: &InvestigationPackageManifestDto,
) -> Result<Vec<PluginExecutionResultDto>, String> {
    let plugin_states = state.list_plugins()?;
    let mut results = Vec::new();
    for plugin in plugin_states.into_iter().filter(|item| item.enabled) {
        match plugin.plugin_id.as_str() {
            "size-posture-analyzer" => {
                let result = execute_built_in_plugin(
                    state,
                    &plugin.plugin_id,
                    PluginExecutionRequestDto {
                        context_kind: request.source_context.clone(),
                        run_id: request.run_id,
                        left_run_id: request.compare.as_ref().map(|item| item.left_run_id),
                        right_run_id: request.compare.as_ref().map(|item| item.right_run_id),
                        build_id: None,
                        history_query: request.history_query.clone(),
                        range_query: request.range_query.clone(),
                        regression_query: request.regression_query.clone(),
                        inspector_query: request.inspector_query.clone(),
                        inspector_selection: request.inspector_selection.clone(),
                        package_path: None,
                    },
                );
                if let Ok(result) = result {
                    results.push(result);
                }
            }
            "package-provenance-exporter" => {
                results.push(PluginExecutionResultDto {
                    plugin_id: plugin.plugin_id.clone(),
                    status: "ok".to_string(),
                    summary: format!("Package will include {} items and omit {} items.", manifest.included_items.len(), manifest.omitted_items.len()),
                    warnings: manifest.missing_resources.clone(),
                    output_items: vec![
                        PluginOutputItemDto {
                            kind: "package".to_string(),
                            title: "Included items".to_string(),
                            summary: manifest.included_items.iter().map(|item| item.title.clone()).take(4).collect::<Vec<_>>().join(", "),
                            detail: Some(format!("{} included entries", manifest.included_items.len())),
                        },
                    ],
                });
            }
            "timeline-signal-adapter" => {
                let request = if let Some(range_query) = request.range_query.clone() {
                    Some(PluginExecutionRequestDto {
                        context_kind: "range".to_string(),
                        range_query: Some(range_query),
                        ..PluginExecutionRequestDto::default()
                    })
                } else {
                    request.history_query.clone().map(|history_query| PluginExecutionRequestDto {
                        context_kind: "history".to_string(),
                        history_query: Some(history_query),
                        ..PluginExecutionRequestDto::default()
                    })
                };
                if let Some(request) = request {
                    if let Ok(result) = execute_built_in_plugin(state, &plugin.plugin_id, request) {
                        results.push(result);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(results)
}

fn write_investigation_package_bundle(bundle: &InvestigationPackageBundle) -> Result<(), String> {
    fs::create_dir_all(&bundle.package_path)
        .map_err(|err| format!("failed to create package directory '{}': {err}", bundle.package_path.display()))?;
    for file in &bundle.files {
        let path = bundle.package_path.join(&file.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create package directory '{}': {err}", parent.display()))?;
        }
        fs::write(&path, &file.bytes).map_err(|err| format!("failed to write package item '{}': {err}", path.display()))?;
    }
    Ok(())
}

fn load_investigation_package(path: &str) -> Result<OpenInvestigationPackageResultDto, String> {
    let root = resolve_package_root(path);
    let manifest_path = root.join("manifest.json");
    let manifest_raw = fs::read_to_string(&manifest_path)
        .map_err(|err| format!("failed to read package manifest '{}': {err}", manifest_path.display()))?;
    let manifest: InvestigationPackageManifestDto = serde_json::from_str(&manifest_raw)
        .map_err(|err| format!("invalid package manifest '{}': {err}", manifest_path.display()))?;
    if manifest.schema_version != INVESTIGATION_PACKAGE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported package schema version {} (expected {})",
            manifest.schema_version, INVESTIGATION_PACKAGE_SCHEMA_VERSION
        ));
    }
    let summary = summarize_package_manifest(None, &root, &manifest);
    Ok(OpenInvestigationPackageResultDto {
        summary,
        manifest: manifest.clone(),
        dashboard_summary: read_optional_json(root.join("dashboard-summary.json"))?,
        run_detail: read_optional_json(root.join("run-detail.json"))?,
        diff_result: read_optional_json(root.join("diff-result.json"))?,
        timeline: read_optional_json(root.join("timeline.json"))?,
        range_diff: read_optional_json(root.join("range-diff.json"))?,
        regression_result: read_optional_json(root.join("regression-result.json"))?,
        inspector_summary: read_optional_json(root.join("inspector-summary.json"))?,
        inspector_detail: read_optional_json(root.join("inspector-detail.json"))?,
        inspector_source_context: read_optional_json(root.join("inspector-source-context.json"))?,
        policy_document: read_optional_json(root.join("policy-document.json"))?,
    })
}

fn summarize_package_manifest(
    package_id: Option<i64>,
    package_path: &Path,
    manifest: &InvestigationPackageManifestDto,
) -> InvestigationPackageSummaryDto {
    InvestigationPackageSummaryDto {
        package_id,
        package_name: manifest.package_name.clone(),
        package_path: package_path.to_string_lossy().to_string(),
        created_at: manifest.created_at.clone(),
        source_context: manifest.source_context.clone(),
        project_id: manifest.project_id,
        project_name: manifest.project_name.clone(),
        fwmap_version: manifest.fwmap_version.clone(),
        schema_version: manifest.schema_version,
        included_count: manifest.included_items.len(),
        omitted_count: manifest.omitted_items.len(),
        notes: manifest.notes.clone(),
        warnings: manifest.missing_resources.clone(),
    }
}

fn map_recent_package_summary(item: StoredPackageRecord) -> InvestigationPackageSummaryDto {
    InvestigationPackageSummaryDto {
        package_id: Some(item.package_id),
        package_name: item.package_name,
        package_path: item.package_path,
        created_at: item.created_at,
        source_context: item.source_context,
        project_id: item.project_id,
        project_name: None,
        fwmap_version: item.fwmap_version,
        schema_version: item.schema_version,
        included_count: 0,
        omitted_count: 0,
        notes: item.note,
        warnings: Vec::new(),
    }
}

fn resolve_package_root(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_file() && path.file_name().and_then(|item| item.to_str()) == Some("manifest.json") {
        path.parent().map(Path::to_path_buf).unwrap_or(path)
    } else {
        path
    }
}

fn read_optional_json<T: for<'de> serde::Deserialize<'de>>(path: PathBuf) -> Result<Option<T>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read package item '{}': {err}", path.display()))?;
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|err| format!("failed to parse package item '{}': {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fwmap::core::history::{HistoryRecordInput, record_build};
    use fwmap::core::model::{
        AnalysisResult, BinaryInfo, DebugArtifactInfo, DebugInfoSummary, MemorySummary, ObjectContribution, ObjectSourceKind,
        RustAggregate, RustContext, RustFamilyKind, RustFamilySummary, RustSymbolSummary, RustView, SectionCategory, SectionTotal,
        SourceFile, SymbolInfo, ToolchainInfo, ToolchainKind, ToolchainSelection, UnknownSourceBucket, WarningItem, WarningLevel,
        WarningSource,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn desktop_metric_deltas_read_pooled_history_strings() {
        let base = temp_test_dir("desktop-pooled-deltas");
        let state = DesktopState::new(&base).unwrap();
        let db = state.storage.paths().history_db_path.clone();

        record_build(
            &db,
            HistoryRecordInput {
                analysis: sample_analysis(100, 20, 1, 48, "tokio"),
                metadata: BTreeMap::new(),
            },
        )
        .unwrap();
        record_build(
            &db,
            HistoryRecordInput {
                analysis: sample_analysis(160, 24, 2, 96, "tokio"),
                metadata: BTreeMap::new(),
            },
        )
        .unwrap();

        let object_deltas = load_metric_deltas(&db, 1, 2, MetricTable::Object, 10).unwrap();
        let source_deltas = load_metric_deltas(&db, 1, 2, MetricTable::SourceFile, 10).unwrap();
        let symbol_deltas = load_metric_deltas(&db, 1, 2, MetricTable::Symbol, 10).unwrap();
        let rust_deltas = load_metric_deltas(&db, 1, 2, MetricTable::RustDependency, 10).unwrap();

        assert_eq!(object_deltas[0].name, "build/main.o");
        assert_eq!(object_deltas[0].delta, 60);
        assert_eq!(source_deltas[0].name, "src/main.cpp");
        assert_eq!(source_deltas[0].delta, 60);
        assert_eq!(symbol_deltas[0].name, "_Z4mainv");
        assert_eq!(symbol_deltas[0].delta, 60);
        assert_eq!(rust_deltas[0].name, "tokio");
        assert_eq!(rust_deltas[0].delta, 48);

        let growth = load_dashboard_top_growth(&db, 1, 2).unwrap();
        assert!(growth.iter().any(|item| item.name == "src/main.cpp" && item.delta == 60));
        assert!(growth.iter().any(|item| item.name == "_Z4mainv" && item.delta == 60));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn desktop_run_detail_reads_pooled_history_content() {
        let base = temp_test_dir("desktop-run-detail");
        let state = DesktopState::new(&base).unwrap();
        let db = state.storage.paths().history_db_path.clone();

        let build_id = record_build(
            &db,
            HistoryRecordInput {
                analysis: sample_analysis(144, 32, 2, 72, "tokio"),
                metadata: BTreeMap::new(),
            },
        )
        .unwrap();

        let run_id = state
            .storage
            .insert_recent_run(&InsertRunRecord {
                project_id: None,
                build_id,
                created_at: "2026-03-15T20:00:00+09:00".to_string(),
                label: Some("pooled run".to_string()),
                status: "finished".to_string(),
                git_revision: Some("abc1234".to_string()),
                profile: Some("release".to_string()),
                target: Some("x86_64-unknown-linux-gnu".to_string()),
                rom_bytes: 144,
                ram_bytes: 32,
                warning_count: 2,
                history_db_path: db.to_string_lossy().to_string(),
                report_html_path: None,
                report_json_path: None,
            })
            .unwrap();

        let detail = state.run_detail(run_id).unwrap().unwrap();
        assert_eq!(detail.top_symbols[0].0, "main()");
        assert_eq!(detail.top_sections[0].0, ".text");
        assert_eq!(detail.warnings[0].0, "SIZE001");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn desktop_metric_deltas_support_legacy_and_pooled_rows_together() {
        let base = temp_test_dir("desktop-legacy-pooled");
        let state = DesktopState::new(&base).unwrap();
        let db = state.storage.paths().history_db_path.clone();

        let pooled_build_id = record_build(
            &db,
            HistoryRecordInput {
                analysis: sample_analysis(100, 20, 1, 48, "tokio"),
                metadata: BTreeMap::new(),
            },
        )
        .unwrap();
        assert_eq!(pooled_build_id, 1);

        let conn = Connection::open(&db).unwrap();
        conn.execute(
            "INSERT INTO builds (
                id, created_at, elf_path, arch, linker_family, map_format, rom_bytes, ram_bytes, warning_count, error_count, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![2i64, 2i64, "legacy.elf", "ARM", "gnu", "unknown", 180i64, 28i64, 1i64, 0i64, "{}"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO object_metrics (build_id, object_path, size_bytes) VALUES (?1, ?2, ?3)",
            params![2i64, "build/main.o", 180i64],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO source_file_metrics (build_id, path, display_path, directory, size_bytes, function_count, line_range_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![2i64, "src/main.cpp", "src/main.cpp", "src", 180i64, 1i64, 1i64],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO symbol_metrics (build_id, name, demangled_name, size_bytes) VALUES (?1, ?2, ?3, ?4)",
            params![2i64, "_Z4mainv", "main()", 180i64],
        )
        .unwrap();
        drop(conn);

        let object_deltas = load_metric_deltas(&db, 1, 2, MetricTable::Object, 10).unwrap();
        let source_deltas = load_metric_deltas(&db, 1, 2, MetricTable::SourceFile, 10).unwrap();
        let symbol_deltas = load_metric_deltas(&db, 1, 2, MetricTable::Symbol, 10).unwrap();

        assert_eq!(object_deltas[0].name, "build/main.o");
        assert_eq!(object_deltas[0].delta, 80);
        assert_eq!(source_deltas[0].name, "src/main.cpp");
        assert_eq!(source_deltas[0].delta, 80);
        assert_eq!(symbol_deltas[0].name, "_Z4mainv");
        assert_eq!(symbol_deltas[0].delta, 80);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn plugin_registry_lists_builtin_plugins_and_extension_points() {
        let base = temp_test_dir("desktop-plugins");
        let state = DesktopState::new(&base).unwrap();

        let plugins = state.list_plugins().unwrap();
        let extension_points = state.list_extension_points().unwrap();

        assert!(plugins.len() >= 2);
        assert!(plugins.iter().any(|item| item.plugin_id == "size-posture-analyzer"));
        assert!(plugins.iter().any(|item| item.plugin_id == "package-provenance-exporter"));
        assert!(extension_points.iter().any(|item| item.extension_point_id == "analyzer.summary"));
        assert!(extension_points.iter().any(|item| item.extension_point_id == "report.package-section"));

        state.set_plugin_enabled("size-posture-analyzer", false).unwrap();
        let detail = state.get_plugin_detail("size-posture-analyzer").unwrap();
        assert!(!detail.summary.enabled);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn investigation_package_roundtrip_preserves_manifest_and_payloads() {
        let base = temp_test_dir("desktop-package-roundtrip");
        let state = DesktopState::new(&base).unwrap();
        let db = state.storage.paths().history_db_path.clone();
        let build_id = record_build(
            &db,
            HistoryRecordInput {
                analysis: sample_analysis(192, 48, 2, 80, "tokio"),
                metadata: BTreeMap::new(),
            },
        )
        .unwrap();
        let run_id = state
            .storage
            .insert_recent_run(&InsertRunRecord {
                project_id: None,
                build_id,
                created_at: "2026-03-15T21:00:00+09:00".to_string(),
                label: Some("package run".to_string()),
                status: "finished".to_string(),
                git_revision: Some("abc1234".to_string()),
                profile: Some("release".to_string()),
                target: Some("x86_64-unknown-linux-gnu".to_string()),
                rom_bytes: 192,
                ram_bytes: 48,
                warning_count: 2,
                history_db_path: db.to_string_lossy().to_string(),
                report_html_path: None,
                report_json_path: None,
            })
            .unwrap();

        let package_dir = base.join("shared-investigation.fwpkg");
        let summary = state
            .create_investigation_package(CreateInvestigationPackageRequestDto {
                project_id: None,
                package_name: "shared-investigation".to_string(),
                destination_path: package_dir.to_string_lossy().to_string(),
                source_context: "run".to_string(),
                include_sections: vec!["run".to_string(), "dashboard".to_string()],
                include_charts_snapshot: true,
                include_policy_snapshot: false,
                include_plugin_results: true,
                include_notes: true,
                notes: Some("Investigate ROM increase before review".to_string()),
                run_id: Some(run_id),
                compare: None,
                history_query: None,
                range_query: None,
                regression_query: None,
                dashboard_query: Some(DashboardQueryDto::default()),
                inspector_query: None,
                inspector_selection: None,
            })
            .unwrap();

        assert_eq!(summary.package_name, "shared-investigation");
        assert!(package_dir.join("manifest.json").exists());
        assert!(package_dir.join("run-detail.json").exists());

        let opened = state.open_investigation_package(&summary.package_path).unwrap();
        assert_eq!(opened.summary.package_name, "shared-investigation");
        assert_eq!(opened.manifest.source_context, "run");
        assert!(opened.run_detail.is_some());
        assert!(opened.manifest.included_items.iter().any(|item| item.relative_path == "run-detail.json"));
        assert!(opened.manifest.plugin_results.iter().any(|item| item.plugin_id == "size-posture-analyzer"));

        let listed = state.list_recent_packages(None, 10).unwrap();
        assert!(listed.iter().any(|item| item.package_name == "shared-investigation"));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn invalid_package_manifest_is_reported_gracefully() {
        let base = temp_test_dir("desktop-invalid-package");
        let package_dir = base.join("broken.fwpkg");
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(package_dir.join("manifest.json"), "{not-json").unwrap();

        let err = load_investigation_package(package_dir.to_string_lossy().as_ref()).unwrap_err();
        assert!(err.contains("invalid package manifest"));

        let _ = fs::remove_dir_all(base);
    }

    fn temp_test_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let dir = std::env::temp_dir().join(format!("fwmap-{label}-{nonce}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_analysis(rom: u64, ram: u64, warnings: usize, dependency_size: u64, dependency_name: &str) -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "sample.elf".to_string(),
                arch: "ARM".to_string(),
                elf_class: "ELF32".to_string(),
                endian: "little-endian".to_string(),
            },
            git: None,
            rust_context: Some(RustContext {
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
            }),
            rust_view: Some(RustView {
                workspace: Some("/workspace/fwmap".to_string()),
                packages: vec![RustAggregate { name: "fwmap".to_string(), size: rom, symbol_count: 1 }],
                targets: vec![RustAggregate { name: "fwmap".to_string(), size: rom, symbol_count: 1 }],
                crates: vec![RustAggregate { name: "fwmap".to_string(), size: rom, symbol_count: 1 }],
                dependency_crates: vec![RustAggregate {
                    name: dependency_name.to_string(),
                    size: dependency_size,
                    symbol_count: 1,
                }],
                source_files: vec![RustAggregate { name: "src/main.rs".to_string(), size: rom, symbol_count: 1 }],
                grouped_families: vec![RustFamilySummary {
                    kind: RustFamilyKind::Function,
                    key: "fwmap::main".to_string(),
                    display_name: "fwmap::main".to_string(),
                    symbol_count: 1,
                    size: rom,
                }],
                symbols: vec![RustSymbolSummary {
                    raw_name: "_Z4mainv".to_string(),
                    demangled_name: Some("main()".to_string()),
                    display_name: "main()".to_string(),
                    package: Some("fwmap".to_string()),
                    target: Some("fwmap".to_string()),
                    crate_name: Some("fwmap".to_string()),
                    dependency_crate: Some(dependency_name.to_string()),
                    source_path: Some("src/main.rs".to_string()),
                    family_kind: RustFamilyKind::Function,
                    family_key: "fwmap::main".to_string(),
                    size: rom,
                    language: fwmap::core::model::SymbolLanguage::Rust,
                }],
                total_rust_size: rom,
            }),
            toolchain: ToolchainInfo {
                requested: ToolchainSelection::Auto,
                detected: None,
                resolved: ToolchainKind::Gnu,
                linker_family: fwmap::core::model::LinkerFamily::Gnu,
                map_format: fwmap::core::model::MapFormat::Unknown,
                parser_warnings_count: 0,
            },
            debug_info: DebugInfoSummary {
                dwarf_mode: fwmap::core::model::DwarfMode::Auto,
                source_lines: fwmap::core::model::SourceLinesMode::All,
                dwarf_used: true,
                cache_hit: false,
                split_dwarf_detected: false,
                split_dwarf_kind: None,
                unknown_source_ratio: 0.0,
                compilation_units: 1,
                line_zero_ranges: 0,
                generated_ranges: 0,
            },
            debug_artifact: DebugArtifactInfo::default(),
            policy: None,
            sections: Vec::new(),
            symbols: vec![SymbolInfo {
                name: "_Z4mainv".to_string(),
                demangled_name: Some("main()".to_string()),
                section_name: Some(".text".to_string()),
                object_path: Some("build/main.o".to_string()),
                addr: 0,
                size: rom,
            }],
            object_contributions: vec![ObjectContribution {
                object_path: "build/main.o".to_string(),
                source_kind: ObjectSourceKind::Object,
                section_name: Some(".text".to_string()),
                size: rom,
            }],
            archive_contributions: Vec::new(),
            archive_pulls: Vec::new(),
            whole_archive_candidates: Vec::new(),
            relocation_references: Vec::new(),
            cross_references: Vec::new(),
            cpp_view: fwmap::core::model::CppView::default(),
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
            source_files: vec![SourceFile {
                path: "src/main.cpp".to_string(),
                display_path: "src/main.cpp".to_string(),
                directory: "src".to_string(),
                size: rom,
                functions: 1,
                line_ranges: 1,
            }],
            line_attributions: Vec::new(),
            line_hotspots: Vec::new(),
            function_attributions: vec![fwmap::core::model::FunctionAttribution {
                raw_name: "_Z4mainv".to_string(),
                demangled_name: Some("main()".to_string()),
                path: Some("src/main.cpp".to_string()),
                size: rom,
                ranges: Vec::new(),
            }],
            unknown_source: UnknownSourceBucket { size: 0, ranges: Vec::new() },
            warnings: (0..warnings)
                .map(|index| WarningItem {
                    level: if index == warnings.saturating_sub(1) && warnings > 1 {
                        WarningLevel::Error
                    } else {
                        WarningLevel::Warn
                    },
                    code: if index == 0 { "SIZE001".to_string() } else { format!("SIZE{:03}", index + 1) },
                    message: "size warning".to_string(),
                    source: WarningSource::Analyze,
                    related: None,
                })
                .collect(),
        }
    }
}
