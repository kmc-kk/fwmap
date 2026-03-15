import { invoke } from "@tauri-apps/api/core";

import type {
  AnalysisRequest,
  DesktopAppInfo,
  DesktopSettings,
  JobStatus,
  RunDetail,
  RunSummary,
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
