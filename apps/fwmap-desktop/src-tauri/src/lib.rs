pub mod commands;
pub mod dto;
pub mod service;
pub mod storage;

use std::io;

use tauri::Manager;

use crate::service::DesktopState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = build_desktop_state(app)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::desktop_get_app_info,
            commands::desktop_get_settings,
            commands::desktop_save_settings,
            commands::desktop_list_projects,
            commands::desktop_create_project,
            commands::desktop_get_active_project,
            commands::desktop_set_active_project,
            commands::desktop_update_project,
            commands::desktop_delete_project,
            commands::desktop_load_policy,
            commands::desktop_validate_policy,
            commands::desktop_save_policy,
            commands::desktop_export_report,
            commands::desktop_list_recent_exports,
            commands::desktop_start_analysis,
            commands::desktop_get_job_status,
            commands::desktop_cancel_job,
            commands::desktop_list_recent_runs,
            commands::desktop_get_run_detail,
            commands::desktop_get_dashboard_summary,
            commands::desktop_list_history,
            commands::desktop_get_timeline,
            commands::desktop_compare_runs,
            commands::desktop_get_range_diff,
            commands::desktop_detect_regression,
            commands::desktop_list_branches,
            commands::desktop_list_tags,
            commands::desktop_list_extension_points,
            commands::desktop_list_plugins,
            commands::desktop_get_plugin_detail,
            commands::desktop_set_plugin_enabled,
            commands::desktop_run_plugin,
            commands::desktop_create_investigation_package,
            commands::desktop_export_package,
            commands::desktop_open_investigation_package,
            commands::desktop_get_investigation_package_summary,
            commands::desktop_list_recent_packages,
            commands::desktop_get_inspector_summary,
            commands::desktop_get_inspector_breakdown,
            commands::desktop_get_inspector_hierarchy,
            commands::desktop_get_inspector_detail,
            commands::desktop_get_source_context,
            commands::investigation_create,
            commands::investigation_list,
            commands::investigation_get,
            commands::investigation_update,
            commands::investigation_delete,
            commands::investigation_add_evidence,
            commands::investigation_remove_evidence,
            commands::investigation_add_note,
            commands::investigation_update_note,
            commands::investigation_list_timeline,
            commands::investigation_set_verdict,
            commands::investigation_export_package,
        ])
        .run(tauri::generate_context!())
        .expect("error while running fwmap desktop");
}

fn build_desktop_state<R: tauri::Runtime>(app: &tauri::App<R>) -> Result<DesktopState, Box<dyn std::error::Error>> {
    let mut candidates = Vec::new();

    if let Ok(path) = app.path().app_local_data_dir() {
        candidates.push(path);
    }
    if let Ok(path) = app.path().app_data_dir() {
        if !candidates.iter().any(|item| item == &path) {
            candidates.push(path);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join(".fwmap-desktop-data"));
        }
    }

    let mut errors = Vec::new();
    for candidate in candidates {
        match DesktopState::new(&candidate) {
            Ok(state) => {
                eprintln!("fwmap-desktop: using app data dir {}", candidate.display());
                return Ok(state);
            }
            Err(err) => {
                eprintln!("fwmap-desktop: failed to initialize '{}' : {err}", candidate.display());
                errors.push(format!("{}: {err}", candidate.display()));
            }
        }
    }

    Err(Box::new(io::Error::other(format!(
        "failed to initialize desktop storage in any candidate directory: {}",
        errors.join(" | ")
    ))))
}
