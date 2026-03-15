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
                last_map_path TEXT
            );

            CREATE TABLE IF NOT EXISTS recent_runs (
                run_id INTEGER PRIMARY KEY AUTOINCREMENT,
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
            ",
        )
        .map_err(|err| format!("failed to initialize desktop database: {err}"))?;
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
                build_id, created_at, label, status, git_revision, profile, target,
                rom_bytes, ram_bytes, warning_count, history_db_path, report_html_path, report_json_path
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
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
            "SELECT run_id, build_id, created_at, label, status, git_revision, profile, target,
                    rom_bytes, ram_bytes, warning_count, history_db_path, report_html_path, report_json_path
             FROM recent_runs WHERE run_id = ?1",
            params![run_id],
            |row| {
                Ok(StoredRunRecord {
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
                    history_db_path: row.get(11)?,
                    report_html_path: row.get(12)?,
                    report_json_path: row.get(13)?,
                })
            },
        )
        .optional()
        .map_err(|err| format!("failed to query run detail: {err}"))
    }
}
