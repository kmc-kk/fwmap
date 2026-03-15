use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use fwmap::core::analyze::{AnalyzeOptions, analyze_paths};
use fwmap::core::git::GitOptions;
use fwmap::core::history::{HistoryRecordInput, list_builds, record_build, show_build};
use fwmap::core::model::{DwarfMode, SourceLinesMode};
use fwmap::core::rule_config::{apply_threshold_overrides, load_rule_config};
use fwmap::report::render::{SourceRenderOptions, write_html_report, write_json_report};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::dto::{
    AnalysisRequestDto, DesktopAppInfo, DesktopSettingsDto, JobEventDto, JobStatusDto, RunDetailDto, RunSummaryDto,
};
use crate::storage::{DesktopStorage, InsertRunRecord};

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
        let summary = RunSummaryDto {
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
        };
        let detail = show_build(Path::new(&stored.history_db_path), stored.build_id)?;
        let Some(detail) = detail else {
            return Ok(Some(RunDetailDto {
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
            }));
        };

        Ok(Some(RunDetailDto {
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
        }))
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
        job.progress_message = "Cancellation is not implemented in Phase D1".to_string();
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
