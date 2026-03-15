use tauri::{AppHandle, State};

use crate::dto::{
    ActiveProjectStateDto, AddInvestigationEvidenceRequestDto, AddInvestigationNoteRequestDto, AnalysisRequestDto, CreateInvestigationPackageRequestDto, CreateInvestigationRequestDto, CreateProjectRequestDto, DashboardQueryDto, DashboardSummaryDto,
    DesktopAppInfo, DesktopSettingsDto, ExportInvestigationPackageRequestDto, ExportRequestDto, ExportResultDto, ExtensionPointDto, GitRefDto, HistoryItemDto,
    HistoryQueryDto, InspectorBreakdownDto, InspectorDetailDto, InspectorHierarchyNodeDto, InspectorQueryDto, InspectorSelectionDto, InspectorSummaryDto, InvestigationDetailDto, InvestigationEvidenceDto, InvestigationNoteDto, InvestigationPackageSummaryDto, InvestigationSummaryDto, InvestigationTimelineEventDto, InvestigationVerdictDto, JobStatusDto, OpenInvestigationPackageResultDto, PluginDetailDto, PluginExecutionRequestDto, PluginExecutionResultDto, PluginSummaryDto, PolicyDocumentDto, PolicyValidationResultDto, ProjectDetailDto,
    ProjectSummaryDto, RangeDiffQueryDto, RangeDiffResultDto, RecentExportDto, RegressionQueryDto,
    RegressionResultDto, RunCompareRequestDto, RunCompareResultDto, RunDetailDto, RunSummaryDto, SetInvestigationVerdictRequestDto,
    SourceContextDto, TimelineResultDto, UpdateInvestigationNoteRequestDto, UpdateInvestigationRequestDto, UpdateProjectRequestDto,
};
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
pub fn desktop_save_settings(state: State<'_, DesktopState>, settings: DesktopSettingsDto) -> Result<DesktopSettingsDto, String> {
    state.save_settings(settings)
}

#[tauri::command]
pub fn desktop_list_projects(state: State<'_, DesktopState>) -> Result<Vec<ProjectSummaryDto>, String> {
    state.list_projects()
}

#[tauri::command]
pub fn desktop_create_project(state: State<'_, DesktopState>, request: CreateProjectRequestDto) -> Result<ProjectDetailDto, String> {
    state.create_project(request)
}

#[tauri::command]
pub fn desktop_get_active_project(state: State<'_, DesktopState>) -> Result<ActiveProjectStateDto, String> {
    state.get_active_project()
}

#[tauri::command]
pub fn desktop_set_active_project(state: State<'_, DesktopState>, project_id: Option<i64>) -> Result<ActiveProjectStateDto, String> {
    state.set_active_project(project_id)
}

#[tauri::command]
pub fn desktop_update_project(state: State<'_, DesktopState>, project_id: i64, patch: UpdateProjectRequestDto) -> Result<ProjectDetailDto, String> {
    state.update_project(project_id, patch)
}

#[tauri::command]
pub fn desktop_delete_project(state: State<'_, DesktopState>, project_id: i64) -> Result<(), String> {
    state.delete_project(project_id)
}

#[tauri::command]
pub fn desktop_load_policy(state: State<'_, DesktopState>, project_id: Option<i64>, path: Option<String>) -> Result<PolicyDocumentDto, String> {
    state.load_policy(project_id, path)
}

#[tauri::command]
pub fn desktop_validate_policy(state: State<'_, DesktopState>, document: PolicyDocumentDto) -> Result<PolicyValidationResultDto, String> {
    state.validate_policy(document)
}

#[tauri::command]
pub fn desktop_save_policy(state: State<'_, DesktopState>, document: PolicyDocumentDto) -> Result<PolicyDocumentDto, String> {
    state.save_policy(document)
}

#[tauri::command]
pub fn desktop_export_report(state: State<'_, DesktopState>, request: ExportRequestDto) -> Result<ExportResultDto, String> {
    state.export_report(request)
}

#[tauri::command]
pub fn desktop_list_recent_exports(state: State<'_, DesktopState>, project_id: Option<i64>, limit: Option<usize>) -> Result<Vec<RecentExportDto>, String> {
    state.list_recent_exports(project_id, limit.unwrap_or(20))
}

#[tauri::command]
pub fn desktop_start_analysis(app: AppHandle, state: State<'_, DesktopState>, request: AnalysisRequestDto) -> Result<JobStatusDto, String> {
    state.start_analysis(app, request)
}

#[tauri::command]
pub fn desktop_get_job_status(state: State<'_, DesktopState>, job_id: String) -> Result<Option<JobStatusDto>, String> {
    state.get_job_status(&job_id)
}

#[tauri::command]
pub fn desktop_cancel_job(state: State<'_, DesktopState>, job_id: String) -> Result<Option<JobStatusDto>, String> {
    state.cancel_job(&job_id)
}

#[tauri::command]
pub fn desktop_list_recent_runs(state: State<'_, DesktopState>, limit: Option<usize>, offset: Option<usize>) -> Result<Vec<RunSummaryDto>, String> {
    state.list_recent_runs(limit.unwrap_or(20), offset.unwrap_or(0))
}

#[tauri::command]
pub fn desktop_get_run_detail(state: State<'_, DesktopState>, run_id: i64) -> Result<Option<RunDetailDto>, String> {
    state.run_detail(run_id)
}

#[tauri::command]
pub fn desktop_get_dashboard_summary(state: State<'_, DesktopState>, query: DashboardQueryDto) -> Result<DashboardSummaryDto, String> {
    state.dashboard_summary(query)
}

#[tauri::command]
pub fn desktop_list_history(state: State<'_, DesktopState>, query: HistoryQueryDto) -> Result<Vec<HistoryItemDto>, String> {
    state.list_history(query)
}

#[tauri::command]
pub fn desktop_get_timeline(state: State<'_, DesktopState>, query: HistoryQueryDto) -> Result<TimelineResultDto, String> {
    state.timeline(query)
}

#[tauri::command]
pub fn desktop_compare_runs(state: State<'_, DesktopState>, request: RunCompareRequestDto) -> Result<RunCompareResultDto, String> {
    state.compare_runs(request)
}

#[tauri::command]
pub fn desktop_get_range_diff(state: State<'_, DesktopState>, query: RangeDiffQueryDto) -> Result<RangeDiffResultDto, String> {
    state.get_range_diff(query)
}

#[tauri::command]
pub fn desktop_detect_regression(state: State<'_, DesktopState>, query: RegressionQueryDto) -> Result<RegressionResultDto, String> {
    state.detect_regression(query)
}

#[tauri::command]
pub fn desktop_list_branches(state: State<'_, DesktopState>, repo_path: Option<String>) -> Result<Vec<GitRefDto>, String> {
    state.list_branches(repo_path)
}

#[tauri::command]
pub fn desktop_list_tags(state: State<'_, DesktopState>, repo_path: Option<String>) -> Result<Vec<GitRefDto>, String> {
    state.list_tags(repo_path)
}


#[tauri::command]
pub fn desktop_get_inspector_summary(state: State<'_, DesktopState>, query: InspectorQueryDto) -> Result<InspectorSummaryDto, String> {
    state.get_inspector_summary(query)
}

#[tauri::command]
pub fn desktop_get_inspector_breakdown(state: State<'_, DesktopState>, query: InspectorQueryDto) -> Result<InspectorBreakdownDto, String> {
    state.get_inspector_breakdown(query)
}

#[tauri::command]
pub fn desktop_get_inspector_hierarchy(state: State<'_, DesktopState>, query: InspectorQueryDto) -> Result<Vec<InspectorHierarchyNodeDto>, String> {
    state.get_inspector_hierarchy(query)
}

#[tauri::command]
pub fn desktop_get_inspector_detail(
    state: State<'_, DesktopState>,
    query: InspectorQueryDto,
    selection: InspectorSelectionDto,
) -> Result<InspectorDetailDto, String> {
    state.get_inspector_detail(query, selection)
}

#[tauri::command]
pub fn desktop_get_source_context(
    state: State<'_, DesktopState>,
    query: InspectorQueryDto,
    selection: InspectorSelectionDto,
) -> Result<SourceContextDto, String> {
    state.get_source_context(query, selection)
}


#[tauri::command]
pub fn desktop_list_extension_points(state: State<'_, DesktopState>) -> Result<Vec<ExtensionPointDto>, String> {
    state.list_extension_points()
}

#[tauri::command]
pub fn desktop_list_plugins(state: State<'_, DesktopState>) -> Result<Vec<PluginSummaryDto>, String> {
    state.list_plugins()
}

#[tauri::command]
pub fn desktop_get_plugin_detail(state: State<'_, DesktopState>, plugin_id: String) -> Result<PluginDetailDto, String> {
    state.get_plugin_detail(&plugin_id)
}

#[tauri::command]
pub fn desktop_set_plugin_enabled(state: State<'_, DesktopState>, plugin_id: String, enabled: bool) -> Result<PluginSummaryDto, String> {
    state.set_plugin_enabled(&plugin_id, enabled)
}

#[tauri::command]
pub fn desktop_run_plugin(state: State<'_, DesktopState>, plugin_id: String, request: PluginExecutionRequestDto) -> Result<PluginExecutionResultDto, String> {
    state.run_plugin(&plugin_id, request)
}

#[tauri::command]
pub fn desktop_create_investigation_package(
    state: State<'_, DesktopState>,
    request: CreateInvestigationPackageRequestDto,
) -> Result<InvestigationPackageSummaryDto, String> {
    state.create_investigation_package(request)
}

#[tauri::command]
pub fn desktop_export_package(
    state: State<'_, DesktopState>,
    request: CreateInvestigationPackageRequestDto,
) -> Result<InvestigationPackageSummaryDto, String> {
    state.export_package(request)
}

#[tauri::command]
pub fn desktop_open_investigation_package(
    state: State<'_, DesktopState>,
    path: String,
) -> Result<OpenInvestigationPackageResultDto, String> {
    state.open_investigation_package(&path)
}

#[tauri::command]
pub fn desktop_get_investigation_package_summary(
    state: State<'_, DesktopState>,
    path: String,
) -> Result<InvestigationPackageSummaryDto, String> {
    state.get_investigation_package_summary(&path)
}

#[tauri::command]
pub fn desktop_list_recent_packages(
    state: State<'_, DesktopState>,
    project_id: Option<i64>,
    limit: Option<usize>,
) -> Result<Vec<InvestigationPackageSummaryDto>, String> {
    state.list_recent_packages(project_id, limit.unwrap_or(20))
}


#[tauri::command]
pub fn investigation_create(state: State<'_, DesktopState>, request: CreateInvestigationRequestDto) -> Result<InvestigationDetailDto, String> {
    state.create_investigation(request)
}

#[tauri::command]
pub fn investigation_list(state: State<'_, DesktopState>, archived: Option<bool>) -> Result<Vec<InvestigationSummaryDto>, String> {
    state.list_investigations(archived.unwrap_or(false))
}

#[tauri::command]
pub fn investigation_get(state: State<'_, DesktopState>, investigation_id: i64) -> Result<InvestigationDetailDto, String> {
    state.get_investigation(investigation_id)
}

#[tauri::command]
pub fn investigation_update(state: State<'_, DesktopState>, investigation_id: i64, patch: UpdateInvestigationRequestDto) -> Result<InvestigationDetailDto, String> {
    state.update_investigation(investigation_id, patch)
}

#[tauri::command]
pub fn investigation_delete(state: State<'_, DesktopState>, investigation_id: i64) -> Result<(), String> {
    state.delete_investigation(investigation_id)
}

#[tauri::command]
pub fn investigation_add_evidence(state: State<'_, DesktopState>, investigation_id: i64, request: AddInvestigationEvidenceRequestDto) -> Result<InvestigationEvidenceDto, String> {
    state.add_investigation_evidence(investigation_id, request)
}

#[tauri::command]
pub fn investigation_remove_evidence(state: State<'_, DesktopState>, investigation_id: i64, evidence_id: i64) -> Result<(), String> {
    state.remove_investigation_evidence(investigation_id, evidence_id)
}

#[tauri::command]
pub fn investigation_add_note(state: State<'_, DesktopState>, investigation_id: i64, request: AddInvestigationNoteRequestDto) -> Result<InvestigationNoteDto, String> {
    state.add_investigation_note(investigation_id, request)
}

#[tauri::command]
pub fn investigation_update_note(state: State<'_, DesktopState>, note_id: i64, request: UpdateInvestigationNoteRequestDto) -> Result<InvestigationNoteDto, String> {
    state.update_investigation_note(note_id, request)
}

#[tauri::command]
pub fn investigation_list_timeline(state: State<'_, DesktopState>, investigation_id: i64) -> Result<Vec<InvestigationTimelineEventDto>, String> {
    state.list_investigation_timeline(investigation_id)
}

#[tauri::command]
pub fn investigation_set_verdict(state: State<'_, DesktopState>, investigation_id: i64, request: SetInvestigationVerdictRequestDto) -> Result<InvestigationVerdictDto, String> {
    state.set_investigation_verdict(investigation_id, request)
}

#[tauri::command]
pub fn investigation_export_package(state: State<'_, DesktopState>, request: ExportInvestigationPackageRequestDto) -> Result<InvestigationPackageSummaryDto, String> {
    state.export_investigation_package(request)
}
