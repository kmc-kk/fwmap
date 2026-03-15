use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::model::RustContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveRustArtifactMode {
    Auto,
    Strict,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustInputs {
    pub cargo_metadata: Option<PathBuf>,
    pub cargo_build_json: Option<PathBuf>,
    pub cargo_workspace: Option<PathBuf>,
    pub cargo_target_name: Option<String>,
    pub cargo_package: Option<String>,
    pub cargo_target_kind: Option<String>,
    pub cargo_target_triple: Option<String>,
    pub resolve_artifact: ResolveRustArtifactMode,
    pub allow_target_dir_fallback: bool,
}

impl Default for RustInputs {
    fn default() -> Self {
        Self {
            cargo_metadata: None,
            cargo_build_json: None,
            cargo_workspace: None,
            cargo_target_name: None,
            cargo_package: None,
            cargo_target_kind: None,
            cargo_target_triple: None,
            resolve_artifact: ResolveRustArtifactMode::Off,
            allow_target_dir_fallback: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustResolution {
    pub resolved_elf: Option<PathBuf>,
    pub rust_context: Option<RustContext>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoMetadata {
    workspace_root: String,
    target_directory: String,
    workspace_members: Vec<String>,
    packages: Vec<CargoPackage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoPackage {
    id: String,
    name: String,
    manifest_path: String,
    edition: Option<String>,
    targets: Vec<CargoTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoTarget {
    name: String,
    kind: Vec<String>,
    crate_types: Vec<String>,
    edition: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RustArtifactCandidate {
    package_id: String,
    target_name: String,
    target_kind: Vec<String>,
    crate_types: Vec<String>,
    profile: Option<String>,
    target_triple: Option<String>,
    executable: Option<PathBuf>,
    filenames: Vec<PathBuf>,
    fresh: bool,
}

pub fn has_rust_inputs(inputs: &RustInputs) -> bool {
    inputs.cargo_metadata.is_some()
        || inputs.cargo_build_json.is_some()
        || inputs.cargo_workspace.is_some()
        || inputs.cargo_target_name.is_some()
        || inputs.cargo_package.is_some()
        || inputs.cargo_target_kind.is_some()
        || inputs.cargo_target_triple.is_some()
        || inputs.allow_target_dir_fallback
        || inputs.resolve_artifact != ResolveRustArtifactMode::Off
}

pub fn resolve_rust_inputs(explicit_elf: Option<&Path>, inputs: &RustInputs) -> Result<RustResolution, String> {
    if !has_rust_inputs(inputs) || inputs.resolve_artifact == ResolveRustArtifactMode::Off {
        return Ok(RustResolution {
            resolved_elf: explicit_elf.map(Path::to_path_buf),
            rust_context: None,
        });
    }

    let metadata = load_cargo_metadata(inputs)?;
    let candidates = load_build_candidates(inputs)?;

    let selected_candidate = if let Some(path) = explicit_elf {
        select_context_candidate_for_explicit(path, &candidates, inputs)
    } else {
        resolve_candidate_without_explicit(&candidates, inputs)?
    };

    let resolved_elf = if let Some(path) = explicit_elf {
        Some(path.to_path_buf())
    } else if let Some(candidate) = selected_candidate.as_ref() {
        Some(select_artifact_path(candidate).ok_or_else(|| {
            "Cargo build JSON was provided, but no analyzable executable/library artifact was found. Supply --elf or narrow the Cargo target selection."
                .to_string()
        })?)
    } else if inputs.allow_target_dir_fallback {
        resolve_fallback_artifact(metadata.as_ref(), inputs)?
    } else if metadata.is_some() || inputs.cargo_workspace.is_some() {
        return Err(
            "Cargo metadata was provided, but no build artifact could be resolved. Supply --elf or --cargo-build-json."
                .to_string(),
        );
    } else {
        None
    };

    let used_fallback = explicit_elf.is_none() && selected_candidate.is_none() && resolved_elf.is_some();
    let rust_context = build_rust_context(
        metadata.as_ref(),
        selected_candidate.as_ref(),
        resolved_elf.as_deref(),
        inputs,
        used_fallback,
    )?;
    Ok(RustResolution {
        resolved_elf,
        rust_context,
    })
}

fn load_cargo_metadata(inputs: &RustInputs) -> Result<Option<CargoMetadata>, String> {
    if let Some(path) = inputs.cargo_metadata.as_deref() {
        let raw = fs::read_to_string(path)
            .map_err(|err| format!("failed to read cargo metadata '{}': {err}", path.display()))?;
        return parse_cargo_metadata(&raw).map(Some);
    }
    if let Some(path) = inputs.cargo_workspace.as_deref() {
        let manifest = resolve_manifest_path(path)?;
        let output = Command::new("cargo")
            .arg("metadata")
            .arg("--format-version=1")
            .arg("--no-deps")
            .arg("--manifest-path")
            .arg(&manifest)
            .output()
            .map_err(|err| format!("failed to run cargo metadata for '{}': {err}", manifest.display()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(format!(
                "cargo metadata failed for '{}': {}",
                manifest.display(),
                if stderr.is_empty() { "unknown error".to_string() } else { stderr }
            ));
        }
        let stdout = String::from_utf8(output.stdout)
            .map_err(|err| format!("cargo metadata produced non-UTF8 output: {err}"))?;
        return parse_cargo_metadata(&stdout).map(Some);
    }
    Ok(None)
}

fn load_build_candidates(inputs: &RustInputs) -> Result<Vec<RustArtifactCandidate>, String> {
    let Some(path) = inputs.cargo_build_json.as_deref() else {
        return Ok(Vec::new());
    };
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("failed to read cargo build JSON '{}': {err}", path.display()))?;
    parse_cargo_build_messages(&raw)
}

fn resolve_manifest_path(path: &Path) -> Result<PathBuf, String> {
    if path.file_name().and_then(|item| item.to_str()) == Some("Cargo.toml") {
        return Ok(path.to_path_buf());
    }
    let manifest = path.join("Cargo.toml");
    if manifest.exists() {
        Ok(manifest)
    } else {
        Err(format!(
            "Cargo workspace path '{}' does not contain Cargo.toml",
            path.display()
        ))
    }
}

fn select_context_candidate_for_explicit(
    explicit_elf: &Path,
    candidates: &[RustArtifactCandidate],
    inputs: &RustInputs,
) -> Option<RustArtifactCandidate> {
    let filtered = filter_candidates(candidates, inputs);
    if filtered.is_empty() {
        return None;
    }
    let explicit = normalize_path_string(explicit_elf);
    let exact = filtered
        .iter()
        .filter(|candidate| candidate_paths(candidate).iter().any(|path| normalize_path_string(path) == explicit))
        .cloned()
        .collect::<Vec<_>>();
    if exact.len() == 1 {
        return exact.into_iter().next();
    }
    if filtered.len() == 1 {
        return filtered.into_iter().next();
    }
    None
}

fn resolve_candidate_without_explicit(
    candidates: &[RustArtifactCandidate],
    inputs: &RustInputs,
) -> Result<Option<RustArtifactCandidate>, String> {
    let filtered = filter_candidates(candidates, inputs);
    if filtered.is_empty() {
        return Ok(None);
    }
    let analyzable = filtered
        .iter()
        .filter(|candidate| select_artifact_path(candidate).is_some())
        .cloned()
        .collect::<Vec<_>>();
    if analyzable.len() == 1 {
        return Ok(analyzable.into_iter().next());
    }
    if analyzable.is_empty() {
        return Err(
            "Cargo build JSON was provided, but no analyzable executable/library artifact was found. Supply --elf or enable --allow-target-dir-fallback."
                .to_string(),
        );
    }
    Err(format!(
        "Multiple Rust artifacts matched. Re-run with --cargo-target-name <name> or --cargo-package <name>.\n{}",
        analyzable
            .iter()
            .map(describe_candidate)
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

fn resolve_fallback_artifact(metadata: Option<&CargoMetadata>, inputs: &RustInputs) -> Result<Option<PathBuf>, String> {
    let Some(metadata) = metadata else {
        return Err(
            "Rust fallback search is disabled unless Cargo metadata is available. Supply --cargo-metadata or --cargo-workspace."
                .to_string(),
        );
    };
    let target_dir = PathBuf::from(&metadata.target_directory);
    if !target_dir.exists() {
        return Err(format!(
            "Rust fallback search is enabled, but Cargo target directory does not exist: {}",
            target_dir.display()
        ));
    }
    let mut files = Vec::new();
    collect_files(&target_dir, &mut files)?;
    let mut matching = files
        .into_iter()
        .filter(|path| is_analyzable_artifact(path))
        .filter(|path| match inputs.cargo_target_name.as_deref() {
            Some(name) => path
                .file_name()
                .and_then(|item| item.to_str())
                .map(|item| item.contains(name))
                .unwrap_or(false),
            None => true,
        })
        .filter(|path| match inputs.cargo_target_triple.as_deref() {
            Some(triple) => normalize_path_string(path).contains(triple),
            None => true,
        })
        .collect::<Vec<_>>();
    matching.sort_by(|a, b| normalize_path_string(b).cmp(&normalize_path_string(a)));
    matching.dedup_by(|a, b| normalize_path_string(a) == normalize_path_string(b));
    if matching.len() == 1 {
        return Ok(matching.into_iter().next());
    }
    if matching.is_empty() {
        return Err(
            "Rust fallback search is enabled, but no analyzable target artifact was found under Cargo target directories."
                .to_string(),
        );
    }
    Err(format!(
        "Multiple Rust artifacts matched fallback search. Re-run with --cargo-target-name <name> or --cargo-target-triple <triple>.\n{}",
        matching
            .iter()
            .map(|path| format!("  - {}", path.display()))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(root).map_err(|err| format!("failed to read '{}': {err}", root.display()))? {
        let entry = entry.map_err(|err| format!("failed to inspect '{}': {err}", root.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

fn filter_candidates(candidates: &[RustArtifactCandidate], inputs: &RustInputs) -> Vec<RustArtifactCandidate> {
    candidates
        .iter()
        .filter(|candidate| {
            inputs
                .cargo_package
                .as_deref()
                .map(|value| candidate.package_id == value || package_name_from_id(&candidate.package_id).as_deref() == Some(value))
                .unwrap_or(true)
        })
        .filter(|candidate| {
            inputs
                .cargo_target_name
                .as_deref()
                .map(|value| candidate.target_name == value)
                .unwrap_or(true)
        })
        .filter(|candidate| {
            inputs
                .cargo_target_kind
                .as_deref()
                .map(|value| candidate.target_kind.iter().any(|kind| kind == value))
                .unwrap_or(true)
        })
        .filter(|candidate| {
            inputs
                .cargo_target_triple
                .as_deref()
                .map(|value| {
                    candidate
                        .target_triple
                        .as_deref()
                        .map(|triple| triple == value)
                        .unwrap_or(true)
                })
                .unwrap_or(true)
        })
        .cloned()
        .collect()
}

fn candidate_paths(candidate: &RustArtifactCandidate) -> Vec<PathBuf> {
    let mut values = Vec::new();
    if let Some(path) = candidate.executable.as_ref() {
        values.push(path.clone());
    }
    values.extend(candidate.filenames.clone());
    values
}

fn select_artifact_path(candidate: &RustArtifactCandidate) -> Option<PathBuf> {
    if let Some(path) = candidate.executable.as_ref().filter(|path| is_analyzable_artifact(path)) {
        return Some(path.clone());
    }
    candidate
        .filenames
        .iter()
        .find(|path| is_analyzable_artifact(path))
        .cloned()
}

fn is_analyzable_artifact(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|item| item.to_str()) else {
        return false;
    };
    if file_name.ends_with(".d") || file_name.ends_with(".rlib") || file_name.ends_with(".rmeta") || file_name.ends_with(".o") {
        return false;
    }
    match path.extension().and_then(|item| item.to_str()).map(|item| item.to_ascii_lowercase()) {
        None => true,
        Some(ext) if matches!(ext.as_str(), "elf" | "exe" | "so" | "dylib" | "dll") => true,
        _ => false,
    }
}

fn build_rust_context(
    metadata: Option<&CargoMetadata>,
    candidate: Option<&RustArtifactCandidate>,
    artifact_path: Option<&Path>,
    inputs: &RustInputs,
    used_fallback: bool,
) -> Result<Option<RustContext>, String> {
    if metadata.is_none() && candidate.is_none() && artifact_path.is_none() && inputs.cargo_workspace.is_none() {
        return Ok(None);
    }
    let package = find_package(metadata, candidate, inputs);
    let target = find_target(package, candidate, inputs);
    let metadata_source = if used_fallback {
        "cargo-target-dir-fallback"
    } else if inputs.cargo_build_json.is_some() {
        "cargo-build-json"
    } else if inputs.cargo_metadata.is_some() {
        "cargo-metadata-file"
    } else if inputs.cargo_workspace.is_some() {
        "cargo-metadata-command"
    } else if inputs.allow_target_dir_fallback {
        "cargo-target-dir-fallback"
    } else {
        "explicit-elf"
    };
    let workspace_members = metadata
        .map(|item| item.workspace_members.iter().filter_map(|id| package_name_from_id(id)).collect())
        .unwrap_or_default();
    Ok(Some(RustContext {
        workspace_root: metadata.map(|item| item.workspace_root.clone()),
        manifest_path: package.map(|item| item.manifest_path.clone()),
        package_name: package.map(|item| item.name.clone()).or_else(|| candidate.and_then(|item| package_name_from_id(&item.package_id))),
        package_id: package.map(|item| item.id.clone()).or_else(|| candidate.map(|item| item.package_id.clone())),
        target_name: target.map(|item| item.name.clone()).or_else(|| candidate.map(|item| item.target_name.clone())),
        target_kind: target
            .map(|item| item.kind.clone())
            .or_else(|| candidate.map(|item| item.target_kind.clone()))
            .unwrap_or_default(),
        crate_types: target
            .map(|item| item.crate_types.clone())
            .or_else(|| candidate.map(|item| item.crate_types.clone()))
            .unwrap_or_default(),
        edition: target
            .and_then(|item| item.edition.clone())
            .or_else(|| package.and_then(|item| item.edition.clone())),
        target_triple: inputs
            .cargo_target_triple
            .clone()
            .or_else(|| candidate.and_then(|item| item.target_triple.clone())),
        profile: candidate.and_then(|item| item.profile.clone()),
        artifact_path: artifact_path.map(|item| item.to_string_lossy().replace('\\', "/")),
        metadata_source: metadata_source.to_string(),
        workspace_members,
    }))
}

fn find_package<'a>(
    metadata: Option<&'a CargoMetadata>,
    candidate: Option<&RustArtifactCandidate>,
    inputs: &RustInputs,
) -> Option<&'a CargoPackage> {
    let metadata = metadata?;
    if let Some(value) = inputs.cargo_package.as_deref() {
        if let Some(package) = metadata.packages.iter().find(|item| item.id == value || item.name == value) {
            return Some(package);
        }
    }
    if let Some(candidate) = candidate {
        if let Some(package) = metadata.packages.iter().find(|item| item.id == candidate.package_id) {
            return Some(package);
        }
    }
    if metadata.packages.len() == 1 {
        return metadata.packages.first();
    }
    None
}

fn find_target<'a>(package: Option<&'a CargoPackage>, candidate: Option<&RustArtifactCandidate>, inputs: &RustInputs) -> Option<&'a CargoTarget> {
    let package = package?;
    if let Some(name) = inputs.cargo_target_name.as_deref() {
        if let Some(target) = package.targets.iter().find(|item| item.name == name) {
            return Some(target);
        }
    }
    if let Some(candidate) = candidate {
        if let Some(target) = package.targets.iter().find(|item| item.name == candidate.target_name) {
            return Some(target);
        }
    }
    if package.targets.len() == 1 {
        return package.targets.first();
    }
    None
}

fn describe_candidate(candidate: &RustArtifactCandidate) -> String {
    let artifact = select_artifact_path(candidate)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<no analyzable artifact>".to_string());
    format!(
        "  - package={} target={} kind={} artifact={}",
        package_name_from_id(&candidate.package_id).unwrap_or_else(|| candidate.package_id.clone()),
        candidate.target_name,
        candidate.target_kind.join(","),
        artifact
    )
}

fn normalize_path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn package_name_from_id(id: &str) -> Option<String> {
    let tail = id.rsplit('#').next()?;
    let mut parts = tail.split('@');
    parts.next().map(|item| item.to_string())
}

fn parse_cargo_metadata(raw: &str) -> Result<CargoMetadata, String> {
    let value: CargoMetadataJson =
        serde_json::from_str(raw).map_err(|err| format!("failed to parse cargo metadata JSON: {err}"))?;
    Ok(CargoMetadata {
        workspace_root: value.workspace_root.replace('\\', "/"),
        target_directory: value.target_directory.replace('\\', "/"),
        workspace_members: value.workspace_members,
        packages: value
            .packages
            .into_iter()
            .map(|package| CargoPackage {
                id: package.id,
                name: package.name,
                manifest_path: package.manifest_path.replace('\\', "/"),
                edition: package.edition,
                targets: package
                    .targets
                    .into_iter()
                    .map(|target| CargoTarget {
                        name: target.name,
                        kind: target.kind,
                        crate_types: target.crate_types,
                        edition: target.edition,
                    })
                    .collect(),
            })
            .collect(),
    })
}

fn parse_cargo_build_messages(raw: &str) -> Result<Vec<RustArtifactCandidate>, String> {
    let mut candidates = Vec::new();
    for (line_number, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<CargoBuildMessage>(trimmed) else {
            continue;
        };
        if value.reason != "compiler-artifact" {
            continue;
        }
        let Some(package_id) = value.package_id else {
            return Err(format!("cargo build JSON line {} is missing package_id", line_number + 1));
        };
        let Some(target) = value.target else {
            return Err(format!("cargo build JSON line {} is missing target", line_number + 1));
        };
        candidates.push(RustArtifactCandidate {
            package_id,
            target_name: target.name,
            target_kind: target.kind,
            crate_types: target.crate_types,
            profile: value.profile.map(profile_name),
            target_triple: infer_target_triple(value.executable.as_deref(), &value.filenames),
            executable: value.executable.map(PathBuf::from),
            filenames: value.filenames.into_iter().map(PathBuf::from).collect(),
            fresh: value.fresh.unwrap_or(false),
        });
    }
    Ok(dedup_candidates(candidates))
}

fn dedup_candidates(items: Vec<RustArtifactCandidate>) -> Vec<RustArtifactCandidate> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for item in items {
        let key = format!(
            "{}|{}|{}|{}",
            item.package_id,
            item.target_name,
            item.target_kind.join(","),
            item.executable
                .as_ref()
                .map(|path| normalize_path_string(path))
                .unwrap_or_default()
        );
        if seen.insert(key) {
            result.push(item);
        }
    }
    result
}

fn infer_target_triple(executable: Option<&str>, filenames: &[String]) -> Option<String> {
    executable
        .map(|item| item.to_string())
        .into_iter()
        .chain(filenames.iter().cloned())
        .find_map(|path| {
            let parts = path.replace('\\', "/").split('/').map(str::to_string).collect::<Vec<_>>();
            parts.windows(2).find_map(|window| {
                let parent = &window[0];
                let child = &window[1];
                if parent == "target" && child.contains('-') {
                    Some(child.clone())
                } else {
                    None
                }
            })
        })
}

fn profile_name(profile: CargoArtifactProfile) -> String {
    if profile.test {
        "test".to_string()
    } else if profile.opt_level != "0" {
        "release".to_string()
    } else {
        "debug".to_string()
    }
}

#[derive(Debug, Deserialize)]
struct CargoMetadataJson {
    workspace_root: String,
    target_directory: String,
    #[serde(default)]
    workspace_members: Vec<String>,
    #[serde(default)]
    packages: Vec<CargoPackageJson>,
}

#[derive(Debug, Deserialize)]
struct CargoPackageJson {
    id: String,
    name: String,
    manifest_path: String,
    edition: Option<String>,
    #[serde(default)]
    targets: Vec<CargoTargetJson>,
}

#[derive(Debug, Deserialize)]
struct CargoTargetJson {
    name: String,
    #[serde(default)]
    kind: Vec<String>,
    #[serde(default)]
    crate_types: Vec<String>,
    edition: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CargoBuildMessage {
    reason: String,
    package_id: Option<String>,
    target: Option<CargoBuildTargetJson>,
    executable: Option<String>,
    #[serde(default)]
    filenames: Vec<String>,
    profile: Option<CargoArtifactProfile>,
    fresh: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CargoBuildTargetJson {
    name: String,
    #[serde(default)]
    kind: Vec<String>,
    #[serde(default)]
    crate_types: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CargoArtifactProfile {
    opt_level: String,
    test: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        has_rust_inputs, parse_cargo_build_messages, parse_cargo_metadata, resolve_rust_inputs, ResolveRustArtifactMode,
        RustInputs,
    };
    use std::fs;

    fn fixture(name: &str) -> String {
        fs::read_to_string(format!("tests/fixtures/rust/{name}")).unwrap()
    }

    #[test]
    fn parses_cargo_metadata_fixture() {
        let metadata = parse_cargo_metadata(&fixture("single_crate_metadata.json")).unwrap();
        assert_eq!(metadata.workspace_root, "/workspace/fwmap");
        assert_eq!(metadata.packages[0].name, "fwmap");
        assert_eq!(metadata.packages[0].targets[0].name, "fwmap");
    }

    #[test]
    fn parses_cargo_build_json_fixture() {
        let candidates = parse_cargo_build_messages(&fixture("single_crate_build.jsonl")).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].target_name, "fwmap");
        assert_eq!(candidates[0].profile.as_deref(), Some("release"));
    }

    #[test]
    fn detects_ambiguity_without_filters() {
        let temp = std::env::temp_dir().join("fwmap-rust-ambiguity.jsonl");
        fs::write(&temp, fixture("multi_target_build.jsonl")).unwrap();
        let result = resolve_rust_inputs(
            None,
            &RustInputs {
                cargo_build_json: Some(temp.clone()),
                resolve_artifact: ResolveRustArtifactMode::Auto,
                ..RustInputs::default()
            },
        );
        let error = result.err().unwrap();
        assert!(error.contains("Multiple Rust artifacts matched"));
        let _ = fs::remove_file(temp);
    }

    #[test]
    fn resolves_with_target_filter() {
        let dir = std::env::temp_dir().join("fwmap-rust-resolve-with-filter");
        let _ = fs::create_dir_all(&dir);
        let build_json = dir.join("build.jsonl");
        let metadata_json = dir.join("metadata.json");
        let artifact = dir.join("target/release/app-helper");
        fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        fs::write(&artifact, b"not-an-elf").unwrap();
        fs::write(&build_json, fixture("multi_target_build.jsonl").replace("/tmp/fwmap", &normalize(&dir))).unwrap();
        fs::write(&metadata_json, fixture("workspace_metadata.json").replace("/tmp/fwmap", &normalize(&dir))).unwrap();
        let resolved = resolve_rust_inputs(
            None,
            &RustInputs {
                cargo_build_json: Some(build_json),
                cargo_metadata: Some(metadata_json),
                cargo_target_name: Some("app-helper".to_string()),
                resolve_artifact: ResolveRustArtifactMode::Auto,
                ..RustInputs::default()
            },
        )
        .unwrap();
        assert!(resolved.resolved_elf.unwrap().ends_with("app-helper"));
        assert_eq!(resolved.rust_context.unwrap().target_name.as_deref(), Some("app-helper"));
    }

    #[test]
    fn metadata_only_requires_explicit_resolution_or_fallback() {
        let temp = std::env::temp_dir().join("fwmap-rust-metadata-only.json");
        fs::write(&temp, fixture("single_crate_metadata.json")).unwrap();
        let result = resolve_rust_inputs(
            None,
            &RustInputs {
                cargo_metadata: Some(temp.clone()),
                resolve_artifact: ResolveRustArtifactMode::Auto,
                ..RustInputs::default()
            },
        );
        let error = result.err().unwrap();
        assert!(error.contains("Cargo metadata was provided"));
        let _ = fs::remove_file(temp);
    }

    #[test]
    fn fallback_search_can_resolve_target_dir_artifact() {
        let dir = std::env::temp_dir().join("fwmap-rust-fallback");
        let target = dir.join("target/x86_64-unknown-linux-gnu/release/fwmap");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"not-an-elf").unwrap();
        let metadata_path = dir.join("metadata.json");
        fs::write(
            &metadata_path,
            fixture("single_crate_metadata.json")
                .replace("/workspace/fwmap/target", &normalize(&dir.join("target")))
                .replace("/workspace/fwmap", &normalize(&dir)),
        )
        .unwrap();
        let resolved = resolve_rust_inputs(
            None,
            &RustInputs {
                cargo_metadata: Some(metadata_path),
                cargo_target_name: Some("fwmap".to_string()),
                cargo_target_triple: Some("x86_64-unknown-linux-gnu".to_string()),
                allow_target_dir_fallback: true,
                resolve_artifact: ResolveRustArtifactMode::Auto,
                ..RustInputs::default()
            },
        )
        .unwrap();
        assert_eq!(resolved.rust_context.unwrap().metadata_source, "cargo-target-dir-fallback");
        assert!(resolved.resolved_elf.unwrap().ends_with("fwmap"));
    }

    #[test]
    fn explicit_elf_preserves_context_without_ambiguity_failure() {
        let dir = std::env::temp_dir().join("fwmap-rust-explicit");
        let explicit = dir.join("fwmap");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&explicit, b"not-an-elf").unwrap();
        let build_json = dir.join("build.jsonl");
        fs::write(&build_json, fixture("multi_target_build.jsonl").replace("/tmp/fwmap", &normalize(&dir))).unwrap();
        let resolved = resolve_rust_inputs(
            Some(&explicit),
            &RustInputs {
                cargo_build_json: Some(build_json),
                resolve_artifact: ResolveRustArtifactMode::Auto,
                ..RustInputs::default()
            },
        )
        .unwrap();
        assert_eq!(resolved.resolved_elf.unwrap(), explicit);
    }

    #[test]
    fn target_triple_filter_keeps_host_target_candidates_when_triple_is_unknown() {
        let dir = std::env::temp_dir().join("fwmap-rust-host-triple");
        let _ = fs::create_dir_all(&dir);
        let build_json = dir.join("build.jsonl");
        fs::write(&build_json, fixture("single_crate_build.jsonl").replace("/workspace/fwmap", &normalize(&dir))).unwrap();
        let resolved = resolve_rust_inputs(
            None,
            &RustInputs {
                cargo_build_json: Some(build_json),
                cargo_target_name: Some("fwmap".to_string()),
                cargo_target_triple: Some("x86_64-unknown-linux-gnu".to_string()),
                resolve_artifact: ResolveRustArtifactMode::Auto,
                ..RustInputs::default()
            },
        )
        .unwrap();
        assert!(resolved.resolved_elf.unwrap().ends_with("fwmap"));
    }

    #[test]
    fn reports_inputs_presence() {
        assert!(!has_rust_inputs(&RustInputs::default()));
        assert!(has_rust_inputs(&RustInputs {
            cargo_workspace: Some(".".into()),
            ..RustInputs::default()
        }));
    }

    fn normalize(path: &std::path::Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }
}
