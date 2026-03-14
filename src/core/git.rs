use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::GitMetadata;

#[derive(Debug, Clone, Default)]
pub struct GitOptions {
    pub enabled: bool,
    pub repo_path: Option<PathBuf>,
}

pub fn collect_git_metadata(options: &GitOptions) -> Option<GitMetadata> {
    if !options.enabled {
        return None;
    }

    let repo_root = git_output(options.repo_path.as_deref(), &["rev-parse", "--show-toplevel"])?;
    let commit_hash = git_output(options.repo_path.as_deref(), &["rev-parse", "HEAD"])?;
    let short_commit_hash = git_output(options.repo_path.as_deref(), &["rev-parse", "--short", "HEAD"])?;
    let branch_raw = git_output(options.repo_path.as_deref(), &["rev-parse", "--abbrev-ref", "HEAD"]);
    let branch_name = branch_raw
        .as_deref()
        .and_then(|value| if value == "HEAD" { None } else { Some(value.to_string()) });
    let detached_head = matches!(branch_raw.as_deref(), Some("HEAD"));
    let tag_names = git_output(options.repo_path.as_deref(), &["tag", "--points-at", "HEAD"])
        .map(|value| {
            value
                .lines()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let commit_subject = git_output(options.repo_path.as_deref(), &["log", "-1", "--pretty=%s"]);
    let author_name = git_output(options.repo_path.as_deref(), &["log", "-1", "--pretty=%an"]);
    let author_email = git_output(options.repo_path.as_deref(), &["log", "-1", "--pretty=%ae"]);
    let commit_timestamp = git_output(options.repo_path.as_deref(), &["log", "-1", "--pretty=%cI"]);
    let describe = git_output(options.repo_path.as_deref(), &["describe", "--always", "--tags", "--dirty"]);
    let is_dirty = git_status_has_changes(options.repo_path.as_deref()).unwrap_or(false);

    Some(GitMetadata {
        repo_root,
        commit_hash,
        short_commit_hash,
        branch_name,
        detached_head,
        tag_names,
        commit_subject,
        author_name,
        author_email,
        commit_timestamp,
        describe,
        is_dirty,
    })
}

fn git_output(repo_path: Option<&Path>, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    if let Some(path) = repo_path {
        command.arg("-C").arg(path);
    }
    let output = command.args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed)
}

fn git_status_has_changes(repo_path: Option<&Path>) -> Option<bool> {
    let mut command = Command::new("git");
    if let Some(path) = repo_path {
        command.arg("-C").arg(path);
    }
    let output = command.args(["status", "--porcelain"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    Some(!value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::{collect_git_metadata, GitOptions};
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn returns_none_for_non_repo() {
        let dir = temp_dir("no-repo");
        fs::create_dir_all(&dir).unwrap();
        let metadata = collect_git_metadata(&GitOptions {
            enabled: true,
            repo_path: Some(dir.clone()),
        });
        assert!(metadata.is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn collects_metadata_for_head_commit() {
        let dir = init_repo("git-head");
        let metadata = collect_git_metadata(&GitOptions {
            enabled: true,
            repo_path: Some(dir.clone()),
        })
        .unwrap();
        assert_eq!(metadata.branch_name.as_deref(), Some("main"));
        assert!(!metadata.detached_head);
        assert_eq!(metadata.commit_subject.as_deref(), Some("initial commit"));
        assert_eq!(metadata.author_name.as_deref(), Some("fwmap test"));
        assert_eq!(metadata.author_email.as_deref(), Some("fwmap@example.com"));
        assert!(!metadata.commit_hash.is_empty());
        assert!(!metadata.short_commit_hash.is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_detached_head_and_dirty_state() {
        let dir = init_repo("git-detached");
        let commit = run_git_capture(&dir, &["rev-parse", "HEAD"]);
        run_git(&dir, &["checkout", "--detach", commit.trim()]);
        fs::write(dir.join("tracked.txt"), "changed\n").unwrap();
        let metadata = collect_git_metadata(&GitOptions {
            enabled: true,
            repo_path: Some(dir.clone()),
        })
        .unwrap();
        assert!(metadata.detached_head);
        assert!(metadata.branch_name.is_none());
        assert!(metadata.is_dirty);
        let _ = fs::remove_dir_all(dir);
    }

    fn init_repo(label: &str) -> std::path::PathBuf {
        let dir = temp_dir(label);
        fs::create_dir_all(&dir).unwrap();
        run_git(&dir, &["init"]);
        run_git(&dir, &["config", "user.name", "fwmap test"]);
        run_git(&dir, &["config", "user.email", "fwmap@example.com"]);
        fs::write(dir.join("tracked.txt"), "hello\n").unwrap();
        run_git(&dir, &["add", "tracked.txt"]);
        run_git(&dir, &["commit", "-m", "initial commit"]);
        run_git(&dir, &["branch", "-M", "main"]);
        dir
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git").arg("-C").arg(dir).args(args).status().unwrap();
        assert!(status.success(), "git {:?} failed", args);
    }

    fn run_git_capture(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
        assert!(output.status.success(), "git {:?} failed", args);
        String::from_utf8(output.stdout).unwrap()
    }

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("fwmap-{label}-{nanos}"))
    }
}
