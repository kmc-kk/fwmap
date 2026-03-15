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
            commands::desktop_start_analysis,
            commands::desktop_get_job_status,
            commands::desktop_cancel_job,
            commands::desktop_list_recent_runs,
            commands::desktop_get_run_detail,
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
