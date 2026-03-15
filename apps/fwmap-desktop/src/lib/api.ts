import { invoke } from "@tauri-apps/api/core";

import type {
  ActiveProjectState,
  AnalysisRequest,
  CreateProjectRequest,
  DashboardQuery,
  ExportRequest,
  ExportResult,
  DashboardSummary,
  DesktopAppInfo,
  DesktopSettings,
  GitRef,
  PolicyDocument,
  PolicyValidationResult,
  ProjectDetail,
  ProjectSummary,
  RecentExport,
  HistoryItem,
  HistoryQuery,
  InspectorBreakdown,
  InspectorDetail,
  InspectorHierarchyNode,
  InspectorQuery,
  InspectorSelection,
  InspectorSummary,
  JobStatus,
  RangeDiffQuery,
  RangeDiffResult,
  RegressionQuery,
  RegressionResult,
  RunCompareRequest,
  RunCompareResult,
  RunDetail,
  RunSummary,
  SourceContext,
  TimelineResult,
  UpdateProjectRequest,
} from "./types";

export async function getAppInfo(): Promise<DesktopAppInfo> {
  return invoke("desktop_get_app_info");
}

export async function getSettings(): Promise<DesktopSettings> {
  return invoke("desktop_get_settings");
}

export async function saveSettings(settings: DesktopSettings): Promise<DesktopSettings> {
  return invoke("desktop_save_settings", { settings });
}

export async function listProjects(): Promise<ProjectSummary[]> {
  return invoke("desktop_list_projects");
}

export async function createProject(request: CreateProjectRequest): Promise<ProjectDetail> {
  return invoke("desktop_create_project", { request });
}

export async function getActiveProject(): Promise<ActiveProjectState> {
  return invoke("desktop_get_active_project");
}

export async function setActiveProject(projectId: number | null): Promise<ActiveProjectState> {
  return invoke("desktop_set_active_project", { projectId });
}

export async function updateProject(projectId: number, patch: UpdateProjectRequest): Promise<ProjectDetail> {
  return invoke("desktop_update_project", { projectId, patch });
}

export async function deleteProject(projectId: number): Promise<void> {
  return invoke("desktop_delete_project", { projectId });
}

export async function loadPolicy(projectId?: number | null, path?: string | null): Promise<PolicyDocument> {
  return invoke("desktop_load_policy", { projectId: projectId ?? null, path: path ?? null });
}

export async function validatePolicy(document: PolicyDocument): Promise<PolicyValidationResult> {
  return invoke("desktop_validate_policy", { document });
}

export async function savePolicy(document: PolicyDocument): Promise<PolicyDocument> {
  return invoke("desktop_save_policy", { document });
}

export async function exportReport(request: ExportRequest): Promise<ExportResult> {
  return invoke("desktop_export_report", { request });
}

export async function listRecentExports(projectId?: number | null, limit = 20): Promise<RecentExport[]> {
  return invoke("desktop_list_recent_exports", { projectId: projectId ?? null, limit });
}

export async function startAnalysis(request: AnalysisRequest): Promise<JobStatus> {
  return invoke("desktop_start_analysis", { request });
}

export async function getJobStatus(jobId: string): Promise<JobStatus | null> {
  return invoke("desktop_get_job_status", { jobId });
}

export async function cancelJob(jobId: string): Promise<JobStatus | null> {
  return invoke("desktop_cancel_job", { jobId });
}

export async function listRecentRuns(limit = 20, offset = 0): Promise<RunSummary[]> {
  return invoke("desktop_list_recent_runs", { limit, offset });
}

export async function getRunDetail(runId: number): Promise<RunDetail | null> {
  return invoke("desktop_get_run_detail", { runId });
}

export async function getDashboardSummary(query: DashboardQuery): Promise<DashboardSummary> {
  return invoke("desktop_get_dashboard_summary", { query });
}

export async function listHistory(query: HistoryQuery): Promise<HistoryItem[]> {
  return invoke("desktop_list_history", { query });
}

export async function getTimeline(query: HistoryQuery): Promise<TimelineResult> {
  return invoke("desktop_get_timeline", { query });
}

export async function compareRuns(request: RunCompareRequest): Promise<RunCompareResult> {
  return invoke("desktop_compare_runs", { request });
}

export async function getRangeDiff(query: RangeDiffQuery): Promise<RangeDiffResult> {
  return invoke("desktop_get_range_diff", { query });
}

export async function detectRegression(query: RegressionQuery): Promise<RegressionResult> {
  return invoke("desktop_detect_regression", { query });
}

export async function listBranches(repoPath?: string | null): Promise<GitRef[]> {
  return invoke("desktop_list_branches", { repoPath: repoPath ?? null });
}

export async function listTags(repoPath?: string | null): Promise<GitRef[]> {
  return invoke("desktop_list_tags", { repoPath: repoPath ?? null });
}


export async function getInspectorSummary(query: InspectorQuery): Promise<InspectorSummary> {
  return invoke("desktop_get_inspector_summary", { query });
}

export async function getInspectorBreakdown(query: InspectorQuery): Promise<InspectorBreakdown> {
  return invoke("desktop_get_inspector_breakdown", { query });
}

export async function getInspectorHierarchy(query: InspectorQuery): Promise<InspectorHierarchyNode[]> {
  return invoke("desktop_get_inspector_hierarchy", { query });
}

export async function getInspectorDetail(query: InspectorQuery, selection: InspectorSelection): Promise<InspectorDetail> {
  return invoke("desktop_get_inspector_detail", { query, selection });
}

export async function getSourceContext(query: InspectorQuery, selection: InspectorSelection): Promise<SourceContext> {
  return invoke("desktop_get_source_context", { query, selection });
}
