use std::collections::{BTreeMap, HashMap};
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
    AnalysisRequestDto, ChangedFilesSummaryDto, DeltaEntryDto, DesktopAppInfo, DesktopSettingsDto, FirstRuleViolationSummaryDto,
    GitRefDto, HistoryItemDto, HistoryQueryDto, JobEventDto, JobStatusDto, MetricSummaryDto, RangeDiffQueryDto,
    RangeDiffResultDto, RegressionOriginPointDto, RegressionQueryDto, RegressionResultDto, RegressionWindowRowDto,
    RunCompareRequestDto, RunCompareResultDto, RunDetailDto, RunSummaryDto, TimelineEntryDto, TimelineResultDto,
    WorstCommitSummaryDto,
};
use crate::storage::{DesktopStorage, InsertRunRecord, StoredRunRecord};

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

    pub fn list_recent_runs(&self, limit: usize, offset: usize) -> Result<Vec<RunSummaryDto>, String> {
        self.storage.list_recent_runs(limit, offset)
    }

    pub fn run_detail(&self, run_id: i64) -> Result<Option<RunDetailDto>, String> {
        let Some(stored) = self.storage.get_recent_run(run_id)? else {
            return Ok(None);
        };
        Ok(Some(self.build_run_detail(stored)?))
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
