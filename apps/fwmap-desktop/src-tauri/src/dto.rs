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
