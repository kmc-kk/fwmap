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

#[derive(Debug, Clone)]
pub struct StoredPluginStateRecord {
    pub plugin_id: String,
    pub enabled: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct InsertPluginStateRecord {
    pub plugin_id: String,
    pub enabled: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct StoredPackageRecord {
    pub package_id: i64,
    pub project_id: Option<i64>,
    pub investigation_id: Option<i64>,
    pub created_at: String,
    pub package_name: String,
    pub package_path: String,
    pub source_context: String,
    pub schema_version: i64,
    pub fwmap_version: String,
    pub note: Option<String>,
}


#[derive(Debug, Clone)]
pub struct StoredInvestigationRecord {
    pub investigation_id: i64,
    pub title: String,
    pub project_id: Option<i64>,
    pub workspace_id: Option<String>,
    pub baseline_ref_json: String,
    pub target_ref_json: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived: bool,
    pub evidence_count: i64,
    pub note_count: i64,
}

#[derive(Debug, Clone)]
pub struct InsertInvestigationRecord {
    pub title: String,
    pub project_id: Option<i64>,
    pub workspace_id: Option<String>,
    pub baseline_ref_json: String,
    pub target_ref_json: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct UpdateInvestigationRecord {
    pub title: Option<String>,
    pub baseline_ref_json: Option<String>,
    pub target_ref_json: Option<String>,
    pub status: Option<String>,
    pub updated_at: String,
    pub archived: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct StoredInvestigationEvidenceRecord {
    pub evidence_id: i64,
    pub investigation_id: i64,
    pub evidence_type: String,
    pub title: String,
    pub delta: Option<i64>,
    pub severity: String,
    pub confidence: f64,
    pub source_view: String,
    pub linked_view: Option<String>,
    pub stable_ref_json: String,
    pub snapshot_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct InsertInvestigationEvidenceRecord {
    pub investigation_id: i64,
    pub evidence_type: String,
    pub title: String,
    pub delta: Option<i64>,
    pub severity: String,
    pub confidence: f64,
    pub source_view: String,
    pub linked_view: Option<String>,
    pub stable_ref_json: String,
    pub snapshot_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct StoredInvestigationNoteRecord {
    pub note_id: i64,
    pub investigation_id: i64,
    pub linked_entity_type: Option<String>,
    pub linked_entity_id: Option<String>,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct InsertInvestigationNoteRecord {
    pub investigation_id: i64,
    pub linked_entity_type: Option<String>,
    pub linked_entity_id: Option<String>,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct StoredInvestigationTimelineEventRecord {
    pub event_id: i64,
    pub investigation_id: i64,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct InsertInvestigationTimelineEventRecord {
    pub investigation_id: i64,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct StoredInvestigationVerdictRecord {
    pub investigation_id: i64,
    pub verdict_type: String,
    pub confidence: f64,
    pub summary: String,
    pub supporting_evidence_ids_json: String,
    pub unresolved_questions: String,
    pub next_actions: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct InsertInvestigationVerdictRecord {
    pub investigation_id: i64,
    pub verdict_type: String,
    pub confidence: f64,
    pub summary: String,
    pub supporting_evidence_ids_json: String,
    pub unresolved_questions: String,
    pub next_actions: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct InsertPackageRecord {
    pub project_id: Option<i64>,
    pub investigation_id: Option<i64>,
    pub created_at: String,
    pub package_name: String,
    pub package_path: String,
    pub source_context: String,
    pub schema_version: i64,
    pub fwmap_version: String,
    pub note: Option<String>,
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

            CREATE TABLE IF NOT EXISTS plugin_states (
                plugin_id TEXT PRIMARY KEY,
                enabled INTEGER NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS recent_packages (
                package_id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER,
                investigation_id INTEGER,
                created_at TEXT NOT NULL,
                package_name TEXT NOT NULL,
                package_path TEXT NOT NULL,
                source_context TEXT NOT NULL,
                schema_version INTEGER NOT NULL,
                fwmap_version TEXT NOT NULL,
                note TEXT
            );

            CREATE TABLE IF NOT EXISTS investigations (
                investigation_id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                project_id INTEGER,
                workspace_id TEXT,
                baseline_ref_json TEXT NOT NULL,
                target_ref_json TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS investigation_evidence (
                evidence_id INTEGER PRIMARY KEY AUTOINCREMENT,
                investigation_id INTEGER NOT NULL,
                evidence_type TEXT NOT NULL,
                title TEXT NOT NULL,
                delta INTEGER,
                severity TEXT NOT NULL,
                confidence REAL NOT NULL,
                source_view TEXT NOT NULL,
                linked_view TEXT,
                stable_ref_json TEXT NOT NULL,
                snapshot_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS investigation_notes (
                note_id INTEGER PRIMARY KEY AUTOINCREMENT,
                investigation_id INTEGER NOT NULL,
                linked_entity_type TEXT,
                linked_entity_id TEXT,
                body TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS investigation_timeline_events (
                event_id INTEGER PRIMARY KEY AUTOINCREMENT,
                investigation_id INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS investigation_verdicts (
                investigation_id INTEGER PRIMARY KEY,
                verdict_type TEXT NOT NULL,
                confidence REAL NOT NULL,
                summary TEXT NOT NULL,
                supporting_evidence_ids_json TEXT NOT NULL,
                unresolved_questions TEXT NOT NULL,
                next_actions TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )
        .map_err(|err| format!("failed to initialize desktop database: {err}"))?;
        ensure_column(&conn, "settings", "active_project_id", "INTEGER")?;
        ensure_column(&conn, "recent_runs", "project_id", "INTEGER")?;
        ensure_column(&conn, "recent_packages", "investigation_id", "INTEGER")?;
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
        conn.execute("UPDATE recent_packages SET project_id = NULL WHERE project_id = ?1", params![project_id])
            .map_err(|err| format!("failed to detach project packages: {err}"))?;
        conn.execute("UPDATE investigations SET project_id = NULL WHERE project_id = ?1", params![project_id])
            .map_err(|err| format!("failed to detach project investigations: {err}"))?;
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

    pub fn save_plugin_state(&self, record: &InsertPluginStateRecord) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO plugin_states (plugin_id, enabled, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(plugin_id) DO UPDATE SET enabled = excluded.enabled, updated_at = excluded.updated_at",
            params![record.plugin_id, if record.enabled { 1 } else { 0 }, record.updated_at],
        )
        .map_err(|err| format!("failed to save plugin state: {err}"))?;
        Ok(())
    }

    pub fn get_plugin_state(&self, plugin_id: &str) -> Result<Option<StoredPluginStateRecord>, String> {
        let conn = self.open()?;
        conn.query_row(
            "SELECT plugin_id, enabled, updated_at FROM plugin_states WHERE plugin_id = ?1",
            params![plugin_id],
            |row| {
                Ok(StoredPluginStateRecord {
                    plugin_id: row.get(0)?,
                    enabled: row.get::<_, i64>(1)? != 0,
                    updated_at: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|err| format!("failed to load plugin state: {err}"))
    }

    pub fn list_plugin_states(&self) -> Result<Vec<StoredPluginStateRecord>, String> {
        let conn = self.open()?;
        let mut stmt = conn
            .prepare("SELECT plugin_id, enabled, updated_at FROM plugin_states ORDER BY plugin_id ASC")
            .map_err(|err| format!("failed to prepare plugin states query: {err}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(StoredPluginStateRecord {
                    plugin_id: row.get(0)?,
                    enabled: row.get::<_, i64>(1)? != 0,
                    updated_at: row.get(2)?,
                })
            })
            .map_err(|err| format!("failed to query plugin states: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to collect plugin states: {err}"))
    }

    pub fn insert_recent_package(&self, record: &InsertPackageRecord) -> Result<i64, String> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO recent_packages (
                project_id, investigation_id, created_at, package_name, package_path, source_context, schema_version, fwmap_version, note
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.project_id,
                record.investigation_id,
                record.created_at,
                record.package_name,
                record.package_path,
                record.source_context,
                record.schema_version,
                record.fwmap_version,
                record.note,
            ],
        )
        .map_err(|err| format!("failed to insert recent package: {err}"))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_recent_packages(&self, project_id: Option<i64>, limit: usize) -> Result<Vec<StoredPackageRecord>, String> {
        let conn = self.open()?;
        let sql = if project_id.is_some() {
            "SELECT package_id, project_id, investigation_id, created_at, package_name, package_path, source_context, schema_version, fwmap_version, note
             FROM recent_packages WHERE project_id = ?1 ORDER BY package_id DESC LIMIT ?2"
        } else {
            "SELECT package_id, project_id, investigation_id, created_at, package_name, package_path, source_context, schema_version, fwmap_version, note
             FROM recent_packages ORDER BY package_id DESC LIMIT ?1"
        };
        let mut stmt = conn
            .prepare(sql)
            .map_err(|err| format!("failed to prepare recent packages query: {err}"))?;
        let mapper = |row: &rusqlite::Row<'_>| {
            Ok(StoredPackageRecord {
                package_id: row.get(0)?,
                project_id: row.get(1)?,
                investigation_id: row.get(2)?,
                created_at: row.get(3)?,
                package_name: row.get(4)?,
                package_path: row.get(5)?,
                source_context: row.get(6)?,
                schema_version: row.get(7)?,
                fwmap_version: row.get(8)?,
                note: row.get(9)?,
            })
        };
        let rows = if let Some(project_id) = project_id {
            stmt.query_map(params![project_id, limit as i64], mapper)
                .map_err(|err| format!("failed to query recent packages: {err}"))?
        } else {
            stmt.query_map(params![limit as i64], mapper)
                .map_err(|err| format!("failed to query recent packages: {err}"))?
        };
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to collect recent packages: {err}"))
    }

    pub fn insert_investigation(&self, record: &InsertInvestigationRecord) -> Result<i64, String> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO investigations (title, project_id, workspace_id, baseline_ref_json, target_ref_json, status, created_at, updated_at, archived) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
            params![record.title, record.project_id, record.workspace_id, record.baseline_ref_json, record.target_ref_json, record.status, record.created_at, record.updated_at],
        ).map_err(|err| format!("failed to insert investigation: {err}"))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_investigations(&self, archived: bool) -> Result<Vec<StoredInvestigationRecord>, String> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT i.investigation_id, i.title, i.project_id, i.workspace_id, i.baseline_ref_json, i.target_ref_json, i.status, i.created_at, i.updated_at, i.archived,
                    (SELECT COUNT(*) FROM investigation_evidence e WHERE e.investigation_id = i.investigation_id) AS evidence_count,
                    (SELECT COUNT(*) FROM investigation_notes n WHERE n.investigation_id = i.investigation_id) AS note_count
             FROM investigations i
             WHERE i.archived = ?1
             ORDER BY i.updated_at DESC, i.investigation_id DESC"
        ).map_err(|err| format!("failed to prepare investigations query: {err}"))?;
        let rows = stmt.query_map(params![if archived { 1 } else { 0 }], |row| {
            Ok(StoredInvestigationRecord {
                investigation_id: row.get(0)?,
                title: row.get(1)?,
                project_id: row.get(2)?,
                workspace_id: row.get(3)?,
                baseline_ref_json: row.get(4)?,
                target_ref_json: row.get(5)?,
                status: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                archived: row.get::<_, i64>(9)? != 0,
                evidence_count: row.get(10)?,
                note_count: row.get(11)?,
            })
        }).map_err(|err| format!("failed to query investigations: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect investigations: {err}"))
    }

    pub fn get_investigation(&self, investigation_id: i64) -> Result<Option<StoredInvestigationRecord>, String> {
        let conn = self.open()?;
        conn.query_row(
            "SELECT i.investigation_id, i.title, i.project_id, i.workspace_id, i.baseline_ref_json, i.target_ref_json, i.status, i.created_at, i.updated_at, i.archived,
                    (SELECT COUNT(*) FROM investigation_evidence e WHERE e.investigation_id = i.investigation_id) AS evidence_count,
                    (SELECT COUNT(*) FROM investigation_notes n WHERE n.investigation_id = i.investigation_id) AS note_count
             FROM investigations i WHERE i.investigation_id = ?1",
            params![investigation_id],
            |row| {
                Ok(StoredInvestigationRecord {
                    investigation_id: row.get(0)?,
                    title: row.get(1)?,
                    project_id: row.get(2)?,
                    workspace_id: row.get(3)?,
                    baseline_ref_json: row.get(4)?,
                    target_ref_json: row.get(5)?,
                    status: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    archived: row.get::<_, i64>(9)? != 0,
                    evidence_count: row.get(10)?,
                    note_count: row.get(11)?,
                })
            },
        ).optional().map_err(|err| format!("failed to load investigation: {err}"))
    }

    pub fn update_investigation(&self, investigation_id: i64, patch: &UpdateInvestigationRecord) -> Result<(), String> {
        let current = self.get_investigation(investigation_id)?.ok_or_else(|| format!("investigation {investigation_id} was not found"))?;
        let conn = self.open()?;
        conn.execute(
            "UPDATE investigations SET title = ?2, baseline_ref_json = ?3, target_ref_json = ?4, status = ?5, updated_at = ?6, archived = ?7 WHERE investigation_id = ?1",
            params![
                investigation_id,
                patch.title.as_ref().unwrap_or(&current.title),
                patch.baseline_ref_json.as_ref().unwrap_or(&current.baseline_ref_json),
                patch.target_ref_json.as_ref().unwrap_or(&current.target_ref_json),
                patch.status.as_ref().unwrap_or(&current.status),
                patch.updated_at,
                patch.archived.map(|v| if v { 1 } else { 0 }).unwrap_or(if current.archived { 1 } else { 0 }),
            ],
        ).map_err(|err| format!("failed to update investigation: {err}"))?;
        Ok(())
    }

    pub fn delete_investigation(&self, investigation_id: i64) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute("DELETE FROM investigation_verdicts WHERE investigation_id = ?1", params![investigation_id]).map_err(|err| format!("failed to delete investigation verdict: {err}"))?;
        conn.execute("DELETE FROM investigation_timeline_events WHERE investigation_id = ?1", params![investigation_id]).map_err(|err| format!("failed to delete investigation timeline: {err}"))?;
        conn.execute("DELETE FROM investigation_notes WHERE investigation_id = ?1", params![investigation_id]).map_err(|err| format!("failed to delete investigation notes: {err}"))?;
        conn.execute("DELETE FROM investigation_evidence WHERE investigation_id = ?1", params![investigation_id]).map_err(|err| format!("failed to delete investigation evidence: {err}"))?;
        conn.execute("UPDATE recent_packages SET investigation_id = NULL WHERE investigation_id = ?1", params![investigation_id]).map_err(|err| format!("failed to detach investigation packages: {err}"))?;
        conn.execute("DELETE FROM investigations WHERE investigation_id = ?1", params![investigation_id]).map_err(|err| format!("failed to delete investigation: {err}"))?;
        Ok(())
    }

    pub fn insert_investigation_evidence(&self, record: &InsertInvestigationEvidenceRecord) -> Result<i64, String> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO investigation_evidence (investigation_id, evidence_type, title, delta, severity, confidence, source_view, linked_view, stable_ref_json, snapshot_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![record.investigation_id, record.evidence_type, record.title, record.delta, record.severity, record.confidence, record.source_view, record.linked_view, record.stable_ref_json, record.snapshot_json, record.created_at],
        ).map_err(|err| format!("failed to insert investigation evidence: {err}"))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_investigation_evidence(&self, investigation_id: i64) -> Result<Vec<StoredInvestigationEvidenceRecord>, String> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT evidence_id, investigation_id, evidence_type, title, delta, severity, confidence, source_view, linked_view, stable_ref_json, snapshot_json, created_at
             FROM investigation_evidence WHERE investigation_id = ?1 ORDER BY created_at DESC, evidence_id DESC"
        ).map_err(|err| format!("failed to prepare evidence query: {err}"))?;
        let rows = stmt.query_map(params![investigation_id], |row| {
            Ok(StoredInvestigationEvidenceRecord {
                evidence_id: row.get(0)?,
                investigation_id: row.get(1)?,
                evidence_type: row.get(2)?,
                title: row.get(3)?,
                delta: row.get(4)?,
                severity: row.get(5)?,
                confidence: row.get(6)?,
                source_view: row.get(7)?,
                linked_view: row.get(8)?,
                stable_ref_json: row.get(9)?,
                snapshot_json: row.get(10)?,
                created_at: row.get(11)?,
            })
        }).map_err(|err| format!("failed to query evidence: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect evidence: {err}"))
    }

    pub fn remove_investigation_evidence(&self, investigation_id: i64, evidence_id: i64) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute("DELETE FROM investigation_evidence WHERE investigation_id = ?1 AND evidence_id = ?2", params![investigation_id, evidence_id]).map_err(|err| format!("failed to remove evidence: {err}"))?;
        Ok(())
    }

    pub fn insert_investigation_note(&self, record: &InsertInvestigationNoteRecord) -> Result<i64, String> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO investigation_notes (investigation_id, linked_entity_type, linked_entity_id, body, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![record.investigation_id, record.linked_entity_type, record.linked_entity_id, record.body, record.created_at, record.updated_at],
        ).map_err(|err| format!("failed to insert investigation note: {err}"))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_investigation_notes(&self, investigation_id: i64) -> Result<Vec<StoredInvestigationNoteRecord>, String> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT note_id, investigation_id, linked_entity_type, linked_entity_id, body, created_at, updated_at
             FROM investigation_notes WHERE investigation_id = ?1 ORDER BY updated_at DESC, note_id DESC"
        ).map_err(|err| format!("failed to prepare notes query: {err}"))?;
        let rows = stmt.query_map(params![investigation_id], |row| {
            Ok(StoredInvestigationNoteRecord {
                note_id: row.get(0)?,
                investigation_id: row.get(1)?,
                linked_entity_type: row.get(2)?,
                linked_entity_id: row.get(3)?,
                body: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        }).map_err(|err| format!("failed to query notes: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect notes: {err}"))
    }

    pub fn update_investigation_note(&self, note_id: i64, body: &str, updated_at: &str) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute("UPDATE investigation_notes SET body = ?2, updated_at = ?3 WHERE note_id = ?1", params![note_id, body, updated_at]).map_err(|err| format!("failed to update investigation note: {err}"))?;
        Ok(())
    }

    pub fn insert_investigation_timeline_event(&self, record: &InsertInvestigationTimelineEventRecord) -> Result<i64, String> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO investigation_timeline_events (investigation_id, event_type, payload_json, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![record.investigation_id, record.event_type, record.payload_json, record.created_at],
        ).map_err(|err| format!("failed to insert investigation timeline event: {err}"))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_investigation_timeline(&self, investigation_id: i64) -> Result<Vec<StoredInvestigationTimelineEventRecord>, String> {
        let conn = self.open()?;
        let mut stmt = conn.prepare(
            "SELECT event_id, investigation_id, event_type, payload_json, created_at FROM investigation_timeline_events WHERE investigation_id = ?1 ORDER BY event_id DESC"
        ).map_err(|err| format!("failed to prepare investigation timeline query: {err}"))?;
        let rows = stmt.query_map(params![investigation_id], |row| {
            Ok(StoredInvestigationTimelineEventRecord {
                event_id: row.get(0)?,
                investigation_id: row.get(1)?,
                event_type: row.get(2)?,
                payload_json: row.get(3)?,
                created_at: row.get(4)?,
            })
        }).map_err(|err| format!("failed to query investigation timeline: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|err| format!("failed to collect investigation timeline: {err}"))
    }

    pub fn save_investigation_verdict(&self, record: &InsertInvestigationVerdictRecord) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO investigation_verdicts (investigation_id, verdict_type, confidence, summary, supporting_evidence_ids_json, unresolved_questions, next_actions, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(investigation_id) DO UPDATE SET verdict_type = excluded.verdict_type, confidence = excluded.confidence, summary = excluded.summary, supporting_evidence_ids_json = excluded.supporting_evidence_ids_json, unresolved_questions = excluded.unresolved_questions, next_actions = excluded.next_actions, updated_at = excluded.updated_at",
            params![record.investigation_id, record.verdict_type, record.confidence, record.summary, record.supporting_evidence_ids_json, record.unresolved_questions, record.next_actions, record.updated_at],
        ).map_err(|err| format!("failed to save investigation verdict: {err}"))?;
        Ok(())
    }

    pub fn get_investigation_verdict(&self, investigation_id: i64) -> Result<Option<StoredInvestigationVerdictRecord>, String> {
        let conn = self.open()?;
        conn.query_row(
            "SELECT investigation_id, verdict_type, confidence, summary, supporting_evidence_ids_json, unresolved_questions, next_actions, updated_at
             FROM investigation_verdicts WHERE investigation_id = ?1",
            params![investigation_id],
            |row| {
                Ok(StoredInvestigationVerdictRecord {
                    investigation_id: row.get(0)?,
                    verdict_type: row.get(1)?,
                    confidence: row.get(2)?,
                    summary: row.get(3)?,
                    supporting_evidence_ids_json: row.get(4)?,
                    unresolved_questions: row.get(5)?,
                    next_actions: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            }
        ).optional().map_err(|err| format!("failed to load investigation verdict: {err}"))
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
