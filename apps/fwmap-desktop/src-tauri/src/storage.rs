use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};

use crate::dto::{DesktopSettingsDto, RunSummaryDto};

#[derive(Debug, Clone)]
pub struct DesktopPaths {
    pub base_dir: PathBuf,
    pub app_db_path: PathBuf,
    pub history_db_path: PathBuf,
    pub runs_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DesktopStorage {
    paths: DesktopPaths,
}

#[derive(Debug, Clone)]
pub struct StoredRunRecord {
    pub run_id: i64,
    pub project_id: Option<i64>,
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
    pub history_db_path: String,
    pub report_html_path: Option<String>,
    pub report_json_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InsertRunRecord {
    pub project_id: Option<i64>,
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
    pub history_db_path: String,
    pub report_html_path: Option<String>,
    pub report_json_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredProjectRecord {
    pub project_id: i64,
    pub name: String,
    pub root_path: String,
    pub git_repo_path: Option<String>,
    pub default_elf_path: Option<String>,
    pub default_map_path: Option<String>,
    pub default_debug_path: Option<String>,
    pub default_rule_file_path: Option<String>,
    pub default_target: Option<String>,
    pub default_profile: Option<String>,
    pub default_export_dir: Option<String>,
    pub pinned_report_path: Option<String>,
    pub last_opened_screen: Option<String>,
    pub last_opened_filters_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_run_at: Option<String>,
    pub last_export_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InsertProjectRecord {
    pub name: String,
    pub root_path: String,
    pub git_repo_path: Option<String>,
    pub default_elf_path: Option<String>,
    pub default_map_path: Option<String>,
    pub default_debug_path: Option<String>,
    pub default_rule_file_path: Option<String>,
    pub default_target: Option<String>,
    pub default_profile: Option<String>,
    pub default_export_dir: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct UpdateProjectRecord {
    pub name: Option<String>,
    pub root_path: Option<String>,
    pub git_repo_path: Option<String>,
    pub default_elf_path: Option<String>,
    pub default_map_path: Option<String>,
    pub default_debug_path: Option<String>,
    pub default_rule_file_path: Option<String>,
    pub default_target: Option<String>,
    pub default_profile: Option<String>,
    pub default_export_dir: Option<String>,
    pub pinned_report_path: Option<String>,
    pub last_opened_screen: Option<String>,
    pub last_opened_filters_json: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct StoredExportRecord {
    pub export_id: i64,
    pub project_id: Option<i64>,
    pub created_at: String,
    pub export_target: String,
    pub format: String,
    pub destination_path: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct InsertExportRecord {
    pub project_id: Option<i64>,
    pub created_at: String,
    pub export_target: String,
    pub format: String,
    pub destination_path: String,
    pub title: String,
}

impl DesktopStorage {
    pub fn new(base_dir: impl AsRef<Path>) -> Result<Self, String> {
        let base_dir = base_dir.as_ref().to_path_buf();
        let paths = DesktopPaths {
            app_db_path: base_dir.join("desktop.db"),
            history_db_path: base_dir.join("history").join("history.db"),
            runs_dir: base_dir.join("runs"),
            base_dir,
        };
        std::fs::create_dir_all(&paths.base_dir)
            .map_err(|err| format!("failed to create app data dir '{}': {err}", paths.base_dir.display()))?;
        std::fs::create_dir_all(paths.history_db_path.parent().unwrap())
            .map_err(|err| format!("failed to create history dir '{}': {err}", paths.history_db_path.parent().unwrap().display()))?;
        std::fs::create_dir_all(&paths.runs_dir)
            .map_err(|err| format!("failed to create runs dir '{}': {err}", paths.runs_dir.display()))?;

        let storage = Self { paths };
        storage.init()?;
        Ok(storage)
    }

    pub fn paths(&self) -> &DesktopPaths {
        &self.paths
    }

    fn open(&self) -> Result<Connection, String> {
        Connection::open(&self.paths.app_db_path)
            .map_err(|err| format!("failed to open desktop database '{}': {err}", self.paths.app_db_path.display()))
    }

    fn init(&self) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                history_db_path TEXT NOT NULL,
                default_rule_file_path TEXT,
                default_git_repo_path TEXT,
                last_elf_path TEXT,
                last_map_path TEXT,
                active_project_id INTEGER
            );

            CREATE TABLE IF NOT EXISTS projects (
                project_id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                root_path TEXT NOT NULL,
                git_repo_path TEXT,
                default_elf_path TEXT,
                default_map_path TEXT,
                default_debug_path TEXT,
                default_rule_file_path TEXT,
                default_target TEXT,
                default_profile TEXT,
                default_export_dir TEXT,
                pinned_report_path TEXT,
                last_opened_screen TEXT,
                last_opened_filters_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS recent_runs (
                run_id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER,
                build_id INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                label TEXT,
                status TEXT NOT NULL,
                git_revision TEXT,
                profile TEXT,
                target TEXT,
                rom_bytes INTEGER NOT NULL,
                ram_bytes INTEGER NOT NULL,
                warning_count INTEGER NOT NULL,
                history_db_path TEXT NOT NULL,
                report_html_path TEXT,
                report_json_path TEXT
            );

            CREATE TABLE IF NOT EXISTS recent_exports (
                export_id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER,
                created_at TEXT NOT NULL,
                export_target TEXT NOT NULL,
                format TEXT NOT NULL,
                destination_path TEXT NOT NULL,
                title TEXT NOT NULL
            );
            ",
        )
        .map_err(|err| format!("failed to initialize desktop database: {err}"))?;
        ensure_column(&conn, "settings", "active_project_id", "INTEGER")?;
        ensure_column(&conn, "recent_runs", "project_id", "INTEGER")?;
        conn.execute(
            "INSERT INTO settings (id, history_db_path) VALUES (1, ?1) ON CONFLICT(id) DO NOTHING",
            params![self.paths.history_db_path.to_string_lossy().to_string()],
        )
        .map_err(|err| format!("failed to create settings row: {err}"))?;
        conn.execute(
            "UPDATE settings SET history_db_path = COALESCE(NULLIF(history_db_path, ''), ?1) WHERE id = 1",
            params![self.paths.history_db_path.to_string_lossy().to_string()],
        )
        .map_err(|err| format!("failed to seed settings row: {err}"))?;
        Ok(())
    }

    pub fn load_settings(&self) -> Result<DesktopSettingsDto, String> {
        let conn = self.open()?;
        conn.query_row(
            "SELECT history_db_path, default_rule_file_path, default_git_repo_path, last_elf_path, last_map_path FROM settings WHERE id = 1",
            [],
            |row| {
                Ok(DesktopSettingsDto {
                    history_db_path: row.get(0)?,
                    default_rule_file_path: row.get(1)?,
                    default_git_repo_path: row.get(2)?,
                    last_elf_path: row.get(3)?,
                    last_map_path: row.get(4)?,
                })
            },
        )
        .map_err(|err| format!("failed to load desktop settings: {err}"))
    }

    pub fn save_settings(&self, settings: &DesktopSettingsDto) -> Result<(), String> {
        if let Some(parent) = Path::new(&settings.history_db_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create history db directory '{}': {err}", parent.display()))?;
        }
        let conn = self.open()?;
        conn.execute(
            "UPDATE settings SET history_db_path = ?1, default_rule_file_path = ?2, default_git_repo_path = ?3, last_elf_path = ?4, last_map_path = ?5 WHERE id = 1",
            params![
                settings.history_db_path,
                settings.default_rule_file_path,
                settings.default_git_repo_path,
                settings.last_elf_path,
                settings.last_map_path,
            ],
        )
        .map_err(|err| format!("failed to save desktop settings: {err}"))?;
        Ok(())
    }

    pub fn remember_selected_files(&self, elf_path: Option<&str>, map_path: Option<&str>) -> Result<(), String> {
        let mut settings = self.load_settings()?;
        if let Some(path) = elf_path {
            settings.last_elf_path = Some(path.to_string());
        }
        if let Some(path) = map_path {
            settings.last_map_path = Some(path.to_string());
        }
        self.save_settings(&settings)
    }

    pub fn insert_recent_run(&self, run: &InsertRunRecord) -> Result<i64, String> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO recent_runs (
                project_id, build_id, created_at, label, status, git_revision, profile, target,
                rom_bytes, ram_bytes, warning_count, history_db_path, report_html_path, report_json_path
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                run.project_id,
                run.build_id,
                run.created_at,
                run.label,
                run.status,
                run.git_revision,
                run.profile,
                run.target,
                run.rom_bytes as i64,
                run.ram_bytes as i64,
                run.warning_count as i64,
                run.history_db_path,
                run.report_html_path,
                run.report_json_path,
            ],
        )
        .map_err(|err| format!("failed to insert recent run: {err}"))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_recent_runs(&self, limit: usize, offset: usize) -> Result<Vec<RunSummaryDto>, String> {
        let conn = self.open()?;
        let mut stmt = conn
            .prepare(
                "SELECT run_id, build_id, created_at, label, status, git_revision, profile, target, rom_bytes, ram_bytes, warning_count
                 FROM recent_runs
                 ORDER BY run_id DESC
                 LIMIT ?1 OFFSET ?2",
            )
            .map_err(|err| format!("failed to prepare recent runs query: {err}"))?;
        let rows = stmt
            .query_map(params![limit as i64, offset as i64], |row| {
                Ok(RunSummaryDto {
                    run_id: row.get(0)?,
                    build_id: row.get(1)?,
                    created_at: row.get(2)?,
                    label: row.get(3)?,
                    status: row.get(4)?,
                    git_revision: row.get(5)?,
                    profile: row.get(6)?,
                    target: row.get(7)?,
                    rom_bytes: row.get::<_, i64>(8)? as u64,
                    ram_bytes: row.get::<_, i64>(9)? as u64,
                    warning_count: row.get::<_, i64>(10)? as u64,
                })
            })
            .map_err(|err| format!("failed to query recent runs: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to collect recent runs: {err}"))
    }

    pub fn get_recent_run(&self, run_id: i64) -> Result<Option<StoredRunRecord>, String> {
        let conn = self.open()?;
        conn.query_row(
            "SELECT run_id, project_id, build_id, created_at, label, status, git_revision, profile, target,
                    rom_bytes, ram_bytes, warning_count, history_db_path, report_html_path, report_json_path
             FROM recent_runs WHERE run_id = ?1",
            params![run_id],
            |row| {
                Ok(StoredRunRecord {
                    run_id: row.get(0)?,
                    project_id: row.get(1)?,
                    build_id: row.get(2)?,
                    created_at: row.get(3)?,
                    label: row.get(4)?,
                    status: row.get(5)?,
                    git_revision: row.get(6)?,
                    profile: row.get(7)?,
                    target: row.get(8)?,
                    rom_bytes: row.get::<_, i64>(9)? as u64,
                    ram_bytes: row.get::<_, i64>(10)? as u64,
                    warning_count: row.get::<_, i64>(11)? as u64,
                    history_db_path: row.get(12)?,
                    report_html_path: row.get(13)?,
                    report_json_path: row.get(14)?,
                })
            },
        )
        .optional()
        .map_err(|err| format!("failed to query run detail: {err}"))
    }

    pub fn list_projects(&self) -> Result<Vec<StoredProjectRecord>, String> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT p.project_id, p.name, p.root_path, p.git_repo_path, p.default_elf_path, p.default_map_path,
                    p.default_debug_path, p.default_rule_file_path, p.default_target, p.default_profile,
                    p.default_export_dir, p.pinned_report_path, p.last_opened_screen, p.last_opened_filters_json,
                    p.created_at, p.updated_at,
                    (SELECT MAX(created_at) FROM recent_runs rr WHERE rr.project_id = p.project_id) AS last_run_at,
                    (SELECT MAX(created_at) FROM recent_exports re WHERE re.project_id = p.project_id) AS last_export_at
             FROM projects p
             ORDER BY p.updated_at DESC, p.project_id DESC"
        ).map_err(|err| format!("failed to prepare projects query: {err}"))?;
        let rows = stmt.query_map([], map_project_row).map_err(|err| format!("failed to query projects: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect projects: {err}"))
    }

    pub fn get_project(&self, project_id: i64) -> Result<Option<StoredProjectRecord>, String> {
        let conn = self.open()?;
        conn.query_row(
            "SELECT p.project_id, p.name, p.root_path, p.git_repo_path, p.default_elf_path, p.default_map_path,
                    p.default_debug_path, p.default_rule_file_path, p.default_target, p.default_profile,
                    p.default_export_dir, p.pinned_report_path, p.last_opened_screen, p.last_opened_filters_json,
                    p.created_at, p.updated_at,
                    (SELECT MAX(created_at) FROM recent_runs rr WHERE rr.project_id = p.project_id) AS last_run_at,
                    (SELECT MAX(created_at) FROM recent_exports re WHERE re.project_id = p.project_id) AS last_export_at
             FROM projects p WHERE p.project_id = ?1",
            params![project_id],
            map_project_row,
        )
        .optional()
        .map_err(|err| format!("failed to load project: {err}"))
    }

    pub fn insert_project(&self, project: &InsertProjectRecord) -> Result<i64, String> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO projects (
                name, root_path, git_repo_path, default_elf_path, default_map_path, default_debug_path,
                default_rule_file_path, default_target, default_profile, default_export_dir, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                project.name,
                project.root_path,
                project.git_repo_path,
                project.default_elf_path,
                project.default_map_path,
                project.default_debug_path,
                project.default_rule_file_path,
                project.default_target,
                project.default_profile,
                project.default_export_dir,
                project.created_at,
                project.updated_at,
            ],
        ).map_err(|err| format!("failed to insert project: {err}"))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_project(&self, project_id: i64, patch: &UpdateProjectRecord) -> Result<(), String> {
        let current = self.get_project(project_id)?.ok_or_else(|| format!("project {project_id} was not found"))?;
        let conn = self.open()?;
        conn.execute(
            "UPDATE projects SET
                name = ?2,
                root_path = ?3,
                git_repo_path = ?4,
                default_elf_path = ?5,
                default_map_path = ?6,
                default_debug_path = ?7,
                default_rule_file_path = ?8,
                default_target = ?9,
                default_profile = ?10,
                default_export_dir = ?11,
                pinned_report_path = ?12,
                last_opened_screen = ?13,
                last_opened_filters_json = ?14,
                updated_at = ?15
             WHERE project_id = ?1",
            params![
                project_id,
                patch.name.clone().unwrap_or(current.name),
                patch.root_path.clone().unwrap_or(current.root_path),
                patch.git_repo_path.clone().or(current.git_repo_path),
                patch.default_elf_path.clone().or(current.default_elf_path),
                patch.default_map_path.clone().or(current.default_map_path),
                patch.default_debug_path.clone().or(current.default_debug_path),
                patch.default_rule_file_path.clone().or(current.default_rule_file_path),
                patch.default_target.clone().or(current.default_target),
                patch.default_profile.clone().or(current.default_profile),
                patch.default_export_dir.clone().or(current.default_export_dir),
                patch.pinned_report_path.clone().or(current.pinned_report_path),
                patch.last_opened_screen.clone().or(current.last_opened_screen),
                patch.last_opened_filters_json.clone().or(current.last_opened_filters_json),
                patch.updated_at,
            ],
        ).map_err(|err| format!("failed to update project: {err}"))?;
        Ok(())
    }

    pub fn delete_project(&self, project_id: i64) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute("DELETE FROM recent_exports WHERE project_id = ?1", params![project_id])
            .map_err(|err| format!("failed to delete project exports: {err}"))?;
        conn.execute("UPDATE recent_runs SET project_id = NULL WHERE project_id = ?1", params![project_id])
            .map_err(|err| format!("failed to detach project runs: {err}"))?;
        conn.execute("DELETE FROM projects WHERE project_id = ?1", params![project_id])
            .map_err(|err| format!("failed to delete project: {err}"))?;
        conn.execute("UPDATE settings SET active_project_id = NULL WHERE active_project_id = ?1", params![project_id])
            .map_err(|err| format!("failed to clear active project: {err}"))?;
        Ok(())
    }

    pub fn get_active_project_id(&self) -> Result<Option<i64>, String> {
        let conn = self.open()?;
        conn.query_row("SELECT active_project_id FROM settings WHERE id = 1", [], |row| row.get(0))
            .optional()
            .map(|value| value.flatten())
            .map_err(|err| format!("failed to load active project id: {err}"))
    }

    pub fn set_active_project(&self, project_id: Option<i64>) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute("UPDATE settings SET active_project_id = ?1 WHERE id = 1", params![project_id])
            .map_err(|err| format!("failed to save active project: {err}"))?;
        Ok(())
    }

    pub fn insert_recent_export(&self, export: &InsertExportRecord) -> Result<i64, String> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO recent_exports (project_id, created_at, export_target, format, destination_path, title)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                export.project_id,
                export.created_at,
                export.export_target,
                export.format,
                export.destination_path,
                export.title,
            ],
        ).map_err(|err| format!("failed to insert recent export: {err}"))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_recent_exports(&self, project_id: Option<i64>, limit: usize) -> Result<Vec<StoredExportRecord>, String> {
        let conn = self.open()?;
        if let Some(project_id) = project_id {
            let mut stmt = conn.prepare(
                "SELECT export_id, project_id, created_at, export_target, format, destination_path, title
                 FROM recent_exports WHERE project_id = ?1 ORDER BY export_id DESC LIMIT ?2"
            ).map_err(|err| format!("failed to prepare recent exports query: {err}"))?;
            let rows = stmt.query_map(params![project_id, limit as i64], |row| {
                Ok(StoredExportRecord {
                    export_id: row.get(0)?,
                    project_id: row.get(1)?,
                    created_at: row.get(2)?,
                    export_target: row.get(3)?,
                    format: row.get(4)?,
                    destination_path: row.get(5)?,
                    title: row.get(6)?,
                })
            }).map_err(|err| format!("failed to query recent exports: {err}"))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect recent exports: {err}"))
        } else {
            let mut stmt = conn.prepare(
                "SELECT export_id, project_id, created_at, export_target, format, destination_path, title
                 FROM recent_exports ORDER BY export_id DESC LIMIT ?1"
            ).map_err(|err| format!("failed to prepare recent exports query: {err}"))?;
            let rows = stmt.query_map(params![limit as i64], |row| {
                Ok(StoredExportRecord {
                    export_id: row.get(0)?,
                    project_id: row.get(1)?,
                    created_at: row.get(2)?,
                    export_target: row.get(3)?,
                    format: row.get(4)?,
                    destination_path: row.get(5)?,
                    title: row.get(6)?,
                })
            }).map_err(|err| format!("failed to query recent exports: {err}"))?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect recent exports: {err}"))
        }
    }
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<(), String> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).map_err(|err| format!("failed to inspect table {table}: {err}"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1)).map_err(|err| format!("failed to inspect table {table}: {err}"))?;
    let columns = rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to inspect table {table}: {err}"))?;
    if !columns.iter().any(|item| item == column) {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"), [])
            .map_err(|err| format!("failed to migrate table {table}: {err}"))?;
    }
    Ok(())
}

fn map_project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredProjectRecord> {
    Ok(StoredProjectRecord {
        project_id: row.get(0)?,
        name: row.get(1)?,
        root_path: row.get(2)?,
        git_repo_path: row.get(3)?,
        default_elf_path: row.get(4)?,
        default_map_path: row.get(5)?,
        default_debug_path: row.get(6)?,
        default_rule_file_path: row.get(7)?,
        default_target: row.get(8)?,
        default_profile: row.get(9)?,
        default_export_dir: row.get(10)?,
        pinned_report_path: row.get(11)?,
        last_opened_screen: row.get(12)?,
        last_opened_filters_json: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        last_run_at: row.get(16)?,
        last_export_at: row.get(17)?,
    })
}
