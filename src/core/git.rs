use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::GitMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommit {
    pub commit: String,
    pub short_commit: String,
    pub commit_time: String,
    pub author_name: String,
    pub subject: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitOrder {
    Timestamp,
    Ancestry,
}

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

pub fn resolve_repo_root(repo_path: Option<&Path>) -> Option<String> {
    git_output(repo_path, &["rev-parse", "--show-toplevel"])
}

pub fn resolve_revision(repo_path: Option<&Path>, revision: &str) -> Option<String> {
    git_output(repo_path, &["rev-parse", revision])
}

pub fn merge_base(repo_path: Option<&Path>, base: &str, head: &str) -> Option<String> {
    git_output(repo_path, &["merge-base", base, head])
}

pub fn list_commits(repo_path: Option<&Path>, revision: &str, limit: usize, order: CommitOrder) -> Result<Vec<GitCommit>, String> {
    let mut args = vec!["rev-list"];
    if matches!(order, CommitOrder::Ancestry) {
        args.push("--first-parent");
    }
    let limit_value = limit.to_string();
    args.push("--max-count");
    args.push(&limit_value);
    args.push(revision);
    let hashes = git_output_lines(repo_path, &args)?;
    hashes
        .into_iter()
        .map(|commit| {
            read_commit(repo_path, &commit).ok_or_else(|| format!("failed to read commit metadata for '{commit}'"))
        })
        .collect()
}

pub fn list_range_commits(repo_path: Option<&Path>, revision_range: &str, order: CommitOrder) -> Result<Vec<GitCommit>, String> {
    let mut args = vec!["rev-list"];
    if matches!(order, CommitOrder::Ancestry) {
        args.push("--first-parent");
    }
    args.push(revision_range);
    let hashes = git_output_lines(repo_path, &args)?;
    hashes
        .into_iter()
        .map(|commit| {
            read_commit(repo_path, &commit).ok_or_else(|| format!("failed to read commit metadata for '{commit}'"))
        })
        .collect()
}

pub fn changed_files(repo_path: Option<&Path>, base: &str, head: &str) -> Result<Vec<String>, String> {
    git_output_lines(repo_path, &["diff", "--name-only", base, head])
}

fn read_commit(repo_path: Option<&Path>, commit: &str) -> Option<GitCommit> {
    let raw = git_output(
        repo_path,
        &[
            "show",
            "--quiet",
            "--format=%H%x1f%h%x1f%cI%x1f%an%x1f%s",
            commit,
        ],
    )?;
    let mut parts = raw.split('\u{1f}');
    Some(GitCommit {
        commit: parts.next()?.to_string(),
        short_commit: parts.next()?.to_string(),
        commit_time: parts.next()?.to_string(),
        author_name: parts.next()?.to_string(),
        subject: parts.next()?.to_string(),
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

fn git_output_lines(repo_path: Option<&Path>, args: &[&str]) -> Result<Vec<String>, String> {
    let mut command = Command::new("git");
    if let Some(path) = repo_path {
        command.arg("-C").arg(path);
    }
    let output = command
        .args(args)
        .output()
        .map_err(|err| format!("failed to run git {:?}: {err}", args))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("git {:?} failed with status {}", args, output.status)
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
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
    use super::{changed_files, collect_git_metadata, list_commits, list_range_commits, merge_base, resolve_revision, CommitOrder, GitOptions};
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

    #[test]
    fn resolves_revisions_and_lists_ranges() {
        let dir = init_repo("git-range");
        fs::write(dir.join("tracked.txt"), "hello 2\n").unwrap();
        run_git(&dir, &["commit", "-am", "second commit"]);
        let head = resolve_revision(Some(&dir), "HEAD").unwrap();
        let base = merge_base(Some(&dir), "HEAD~1", "HEAD").unwrap();
        assert!(!head.is_empty());
        assert!(!base.is_empty());
        let commits = list_commits(Some(&dir), "HEAD", 10, CommitOrder::Ancestry).unwrap();
        assert_eq!(commits.len(), 2);
        let range = list_range_commits(Some(&dir), "HEAD~1..HEAD", CommitOrder::Timestamp).unwrap();
        assert_eq!(range.len(), 1);
        let files = changed_files(Some(&dir), "HEAD~1", "HEAD").unwrap();
        assert_eq!(files, vec!["tracked.txt".to_string()]);
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
