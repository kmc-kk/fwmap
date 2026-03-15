use tauri::{AppHandle, State};

use crate::dto::{AnalysisRequestDto, DesktopAppInfo, DesktopSettingsDto, JobStatusDto, RunDetailDto, RunSummaryDto};
use crate::service::DesktopState;

#[tauri::command]
pub fn desktop_get_app_info(state: State<'_, DesktopState>) -> Result<DesktopAppInfo, String> {
    state.app_info()
}

#[tauri::command]
pub fn desktop_get_settings(state: State<'_, DesktopState>) -> Result<DesktopSettingsDto, String> {
    state.get_settings()
}

#[tauri::command]
pub fn desktop_save_settings(
    state: State<'_, DesktopState>,
    settings: DesktopSettingsDto,
) -> Result<DesktopSettingsDto, String> {
    state.save_settings(settings)
}

#[tauri::command]
pub fn desktop_start_analysis(
    app: AppHandle,
    state: State<'_, DesktopState>,
    request: AnalysisRequestDto,
) -> Result<JobStatusDto, String> {
    state.start_analysis(app, request)
}

#[tauri::command]
pub fn desktop_get_job_status(
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<Option<JobStatusDto>, String> {
    state.get_job_status(&job_id)
}

#[tauri::command]
pub fn desktop_cancel_job(
    state: State<'_, DesktopState>,
    job_id: String,
) -> Result<Option<JobStatusDto>, String> {
    state.cancel_job(&job_id)
}

#[tauri::command]
pub fn desktop_list_recent_runs(
    state: State<'_, DesktopState>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<RunSummaryDto>, String> {
    state.list_recent_runs(limit.unwrap_or(20), offset.unwrap_or(0))
}

#[tauri::command]
pub fn desktop_get_run_detail(
    state: State<'_, DesktopState>,
    run_id: i64,
) -> Result<Option<RunDetailDto>, String> {
    state.run_detail(run_id)
}
