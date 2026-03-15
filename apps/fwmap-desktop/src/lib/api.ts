import { invoke } from "@tauri-apps/api/core";

import type {
  AnalysisRequest,
  DashboardQuery,
  DashboardSummary,
  DesktopAppInfo,
  DesktopSettings,
  GitRef,
  HistoryItem,
  HistoryQuery,
  JobStatus,
  RangeDiffQuery,
  RangeDiffResult,
  RegressionQuery,
  RegressionResult,
  RunCompareRequest,
  RunCompareResult,
  RunDetail,
  RunSummary,
  TimelineResult,
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
