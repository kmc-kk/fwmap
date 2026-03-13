use std::fs;
use std::path::{Path, PathBuf};

use object::{Object, ObjectSection};

use crate::model::{DebugArtifactInfo, DebugArtifactKind, DebugArtifactSource, DebuginfodMode};

#[derive(Debug, Clone)]
pub struct DebugArtifactResolver {
    pub debug_file_dirs: Vec<PathBuf>,
    pub debuginfod: DebuginfodMode,
    pub debuginfod_urls: Vec<String>,
    pub debuginfod_cache_dir: Option<PathBuf>,
    pub trace: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedDebugArtifact {
    pub info: DebugArtifactInfo,
    pub bytes: Vec<u8>,
}

pub fn resolve_debug_artifact(elf_path: &Path, resolver: &DebugArtifactResolver) -> Result<ResolvedDebugArtifact, String> {
    let bytes = fs::read(elf_path).map_err(|err| format!("failed to read ELF '{}': {err}", elf_path.display()))?;
    let file = object::File::parse(&*bytes).map_err(|err| format!("failed to parse ELF '{}': {err}", elf_path.display()))?;
    let mut steps = Vec::new();
    let build_id = read_build_id(&file);
    let debuglink = read_gnu_debuglink(&file);
    let split_hint = read_split_dwarf_hint(&file);

    if has_embedded_debug(&file) {
        push_step(&mut steps, resolver.trace, format!("embedded debug info was found in {}", elf_path.display()));
        return Ok(ResolvedDebugArtifact {
            info: DebugArtifactInfo {
                kind: DebugArtifactKind::Embedded,
                source: DebugArtifactSource::Embedded,
                path: Some(elf_path.display().to_string()),
                build_id,
                split_dwarf: split_hint.is_some(),
                debuginfod_used: false,
                resolution_steps: steps,
            },
            bytes,
        });
    }

    if let Some(path) = search_user_dirs(elf_path, resolver, debuglink.as_deref(), split_hint.as_deref(), &mut steps) {
        return resolve_sidecar(path, DebugArtifactSource::UserDir, build_id, split_hint.is_some(), steps);
    }
    if let Some(name) = debuglink.as_deref() {
        if let Some(path) = search_debuglink(elf_path, name, resolver, &mut steps) {
            return resolve_sidecar(path, DebugArtifactSource::GnuDebuglink, build_id, split_hint.is_some(), steps);
        }
    }
    if let Some(id) = build_id.as_deref() {
        if let Some(path) = search_build_id(id, resolver, &mut steps) {
            return resolve_sidecar(path, DebugArtifactSource::BuildId, build_id, split_hint.is_some(), steps);
        }
    }
    if let Some(hint) = split_hint.as_deref() {
        if let Some(path) = search_split_dwarf(elf_path, hint, resolver, &mut steps) {
            let kind = split_kind_for_path(&path).unwrap_or(DebugArtifactKind::SplitDwo);
            return resolve_sidecar_with_kind(path, DebugArtifactSource::SplitDwarf, kind, build_id, true, steps);
        }
    }

    handle_debuginfod_fallback(elf_path, &bytes, resolver, build_id, split_hint.is_some(), steps)
}

fn resolve_sidecar(
    path: PathBuf,
    source: DebugArtifactSource,
    build_id: Option<String>,
    split_dwarf: bool,
    steps: Vec<String>,
) -> Result<ResolvedDebugArtifact, String> {
    let kind = split_kind_for_path(&path).unwrap_or(DebugArtifactKind::SeparateDebug);
    resolve_sidecar_with_kind(path, source, kind, build_id, split_dwarf, steps)
}

fn resolve_sidecar_with_kind(
    path: PathBuf,
    source: DebugArtifactSource,
    kind: DebugArtifactKind,
    build_id: Option<String>,
    split_dwarf: bool,
    steps: Vec<String>,
) -> Result<ResolvedDebugArtifact, String> {
    let bytes = fs::read(&path).map_err(|err| format!("failed to read debug artifact '{}': {err}", path.display()))?;
    Ok(ResolvedDebugArtifact {
        info: DebugArtifactInfo {
            kind,
            source,
            path: Some(path.display().to_string()),
            build_id,
            split_dwarf,
            debuginfod_used: false,
            resolution_steps: steps,
        },
        bytes,
    })
}

fn handle_debuginfod_fallback(
    elf_path: &Path,
    elf_bytes: &[u8],
    resolver: &DebugArtifactResolver,
    build_id: Option<String>,
    split_dwarf: bool,
    mut steps: Vec<String>,
) -> Result<ResolvedDebugArtifact, String> {
    match resolver.debuginfod {
        DebuginfodMode::Off => {
            push_step(&mut steps, resolver.trace, "debuginfod is disabled".to_string());
            Ok(ResolvedDebugArtifact {
                info: DebugArtifactInfo {
                    kind: DebugArtifactKind::None,
                    source: DebugArtifactSource::None,
                    path: None,
                    build_id,
                    split_dwarf,
                    debuginfod_used: false,
                    resolution_steps: steps,
                },
                // Keep the main ELF bytes available so downstream DWARF parsing can still
                // explain missing `.debug_line` sections and detect split-DWARF markers.
                bytes: elf_bytes.to_vec(),
            })
        }
        DebuginfodMode::Auto | DebuginfodMode::On => {
            if resolver.debuginfod_urls.is_empty() {
                push_step(
                    &mut steps,
                    resolver.trace,
                    "debuginfod was not attempted because no URL was configured".to_string(),
                );
            } else {
                push_step(
                    &mut steps,
                    resolver.trace,
                    format!(
                        "debuginfod lookup is not implemented yet for {}; continuing without external fetch",
                        elf_path.display()
                    ),
                );
            }
            Ok(ResolvedDebugArtifact {
                info: DebugArtifactInfo {
                    kind: DebugArtifactKind::None,
                    source: if resolver.debuginfod_urls.is_empty() {
                        DebugArtifactSource::None
                    } else {
                        DebugArtifactSource::Debuginfod
                    },
                    path: resolver
                        .debuginfod_cache_dir
                        .as_ref()
                        .map(|path| path.display().to_string()),
                    build_id,
                    split_dwarf,
                    debuginfod_used: false,
                    resolution_steps: steps,
                },
                bytes: elf_bytes.to_vec(),
            })
        }
    }
}

fn has_embedded_debug(file: &object::File<'_>) -> bool {
    file.section_by_name(".debug_line").is_some()
}

fn read_gnu_debuglink(file: &object::File<'_>) -> Option<String> {
    let data = file.section_by_name(".gnu_debuglink")?.uncompressed_data().ok()?;
    let bytes = data.as_ref();
    let name_end = bytes.iter().position(|byte| *byte == 0)?;
    let name = String::from_utf8_lossy(&bytes[..name_end]).into_owned();
    (!name.is_empty()).then_some(name)
}

fn read_build_id(file: &object::File<'_>) -> Option<String> {
    let data = file.section_by_name(".note.gnu.build-id")?.uncompressed_data().ok()?;
    let bytes = data.as_ref();
    if bytes.len() < 16 {
        return None;
    }
    let namesz = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?) as usize;
    let descsz = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) as usize;
    let name_offset = 12usize;
    let name_end = name_offset.checked_add(namesz)?;
    let desc_offset = align4(name_end);
    let desc_end = desc_offset.checked_add(descsz)?;
    let name = bytes.get(name_offset..name_end)?;
    if !name.starts_with(b"GNU") {
        return None;
    }
    Some(hex_encode(bytes.get(desc_offset..desc_end)?))
}

fn read_split_dwarf_hint(file: &object::File<'_>) -> Option<String> {
    if let Some(data) = file.section_by_name(".gnu_debugaltlink").and_then(|section| section.uncompressed_data().ok()) {
        let bytes = data.as_ref();
        let name_end = bytes.iter().position(|byte| *byte == 0)?;
        let name = String::from_utf8_lossy(&bytes[..name_end]).into_owned();
        if !name.is_empty() {
            return Some(name);
        }
    }
    if file.section_by_name(".debug_info.dwo").is_some() {
        return Some(sidecar_name(file, "dwo"));
    }
    if file.section_by_name(".debug_cu_index").is_some() {
        return Some(sidecar_name(file, "dwp"));
    }
    None
}

fn sidecar_name(file: &object::File<'_>, extension: &str) -> String {
    read_build_id(file).unwrap_or_else(|| format!("debug.{extension}"))
}

fn search_user_dirs(
    elf_path: &Path,
    resolver: &DebugArtifactResolver,
    debuglink: Option<&str>,
    split_hint: Option<&str>,
    steps: &mut Vec<String>,
) -> Option<PathBuf> {
    if let Some(name) = debuglink {
        for dir in &resolver.debug_file_dirs {
            let path = dir.join(name);
            push_step(steps, resolver.trace, format!("checking user debug dir {}", path.display()));
            if path.exists() && path != elf_path {
                return Some(path);
            }
        }
    }
    if let Some(name) = split_hint {
        for dir in &resolver.debug_file_dirs {
            let path = dir.join(name);
            push_step(steps, resolver.trace, format!("checking user debug dir {}", path.display()));
            if path.exists() && path != elf_path {
                return Some(path);
            }
        }
    }
    let file_name = elf_path.file_name()?.to_string_lossy().into_owned();
    for dir in &resolver.debug_file_dirs {
        let path = dir.join(&file_name);
        push_step(steps, resolver.trace, format!("checking user debug dir {}", path.display()));
        if path.exists() && path != elf_path {
            return Some(path);
        }
    }
    None
}

fn search_debuglink(elf_path: &Path, debuglink: &str, resolver: &DebugArtifactResolver, steps: &mut Vec<String>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(parent) = elf_path.parent() {
        candidates.push(parent.join(debuglink));
        candidates.push(parent.join(".debug").join(debuglink));
    }
    for dir in &resolver.debug_file_dirs {
        candidates.push(dir.join(debuglink));
    }
    if cfg!(target_family = "unix") {
        candidates.push(PathBuf::from("/usr/lib/debug").join(debuglink.trim_start_matches('/')));
    }
    for candidate in candidates {
        push_step(steps, resolver.trace, format!("checking .gnu_debuglink target {}", candidate.display()));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn search_build_id(build_id: &str, resolver: &DebugArtifactResolver, steps: &mut Vec<String>) -> Option<PathBuf> {
    if build_id.len() < 3 {
        return None;
    }
    let prefix = &build_id[..2];
    let suffix = &build_id[2..];
    let relative = PathBuf::from(".build-id").join(prefix).join(format!("{suffix}.debug"));
    let mut roots = resolver.debug_file_dirs.clone();
    if cfg!(target_family = "unix") {
        roots.push(PathBuf::from("/usr/lib/debug"));
    }
    for root in roots {
        let candidate = root.join(&relative);
        push_step(steps, resolver.trace, format!("checking build-id path {}", candidate.display()));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn search_split_dwarf(
    elf_path: &Path,
    hint: &str,
    resolver: &DebugArtifactResolver,
    steps: &mut Vec<String>,
) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(parent) = elf_path.parent() {
        candidates.push(parent.join(hint));
        candidates.push(parent.join(replace_extension(elf_path, "dwo")));
        candidates.push(parent.join(replace_extension(elf_path, "dwp")));
    }
    for dir in &resolver.debug_file_dirs {
        candidates.push(dir.join(hint));
    }
    for candidate in candidates {
        push_step(steps, resolver.trace, format!("checking split DWARF sidecar {}", candidate.display()));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn replace_extension(path: &Path, extension: &str) -> PathBuf {
    let mut replaced = path
        .file_stem()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("debug"));
    replaced.set_extension(extension);
    replaced
}

fn split_kind_for_path(path: &Path) -> Option<DebugArtifactKind> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("dwo") => Some(DebugArtifactKind::SplitDwo),
        Some("dwp") => Some(DebugArtifactKind::SplitDwp),
        _ => None,
    }
}

fn push_step(steps: &mut Vec<String>, enabled: bool, step: String) {
    if enabled {
        steps.push(step);
    }
}

fn align4(value: usize) -> usize {
    (value + 3) & !3
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{resolve_debug_artifact, DebugArtifactResolver};
    use crate::model::{DebugArtifactKind, DebugArtifactSource, DebuginfodMode};
    use object::write::Object;
    use object::{Architecture, BinaryFormat, Endianness, SectionKind};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn prefers_embedded_debug_info() {
        let dir = temp_dir("embedded");
        fs::create_dir_all(&dir).unwrap();
        let elf = dir.join("app.elf");
        fs::write(&elf, build_object(&[(".debug_line", &[1, 2, 3])], &[])).unwrap();

        let resolved = resolve_debug_artifact(&elf, &resolver(&[])).unwrap();
        assert_eq!(resolved.info.kind, DebugArtifactKind::Embedded);
        assert_eq!(resolved.info.source, DebugArtifactSource::Embedded);
    }

    #[test]
    fn resolves_gnu_debuglink_from_user_dir() {
        let dir = temp_dir("debuglink");
        let debug_dir = dir.join("debug");
        fs::create_dir_all(&debug_dir).unwrap();
        let elf = dir.join("app.elf");
        fs::write(&elf, build_object(&[(".gnu_debuglink", b"app.debug\0\0\0\0")], &[])).unwrap();
        let sidecar = debug_dir.join("app.debug");
        fs::write(&sidecar, build_object(&[(".debug_line", &[9, 9])], &[])).unwrap();

        let resolved = resolve_debug_artifact(&elf, &resolver(&[debug_dir])).unwrap();
        assert_eq!(resolved.info.source, DebugArtifactSource::UserDir);
        assert_eq!(resolved.info.kind, DebugArtifactKind::SeparateDebug);
        assert_eq!(resolved.info.path.as_deref(), Some(sidecar.to_string_lossy().as_ref()));
    }

    #[test]
    fn resolves_build_id_path_from_user_dir() {
        let dir = temp_dir("buildid");
        let debug_dir = dir.join("symbols");
        fs::create_dir_all(debug_dir.join(".build-id").join("ab")).unwrap();
        let elf = dir.join("app.elf");
        fs::write(&elf, build_object(&[(".note.gnu.build-id", &build_id_note(&[0xab, 0xcd, 0xef]))], &[])).unwrap();
        let sidecar = debug_dir.join(".build-id").join("ab").join("cdef.debug");
        fs::write(&sidecar, build_object(&[(".debug_line", &[7, 7])], &[])).unwrap();

        let resolved = resolve_debug_artifact(&elf, &resolver(&[debug_dir])).unwrap();
        assert_eq!(resolved.info.source, DebugArtifactSource::BuildId);
        assert_eq!(resolved.info.build_id.as_deref(), Some("abcdef"));
    }

    #[test]
    fn resolves_split_dwarf_hint_from_user_dir() {
        let dir = temp_dir("split");
        let debug_dir = dir.join("debug");
        fs::create_dir_all(&debug_dir).unwrap();
        let elf = dir.join("app.elf");
        fs::write(&elf, build_object(&[(".gnu_debugaltlink", b"app.dwo\0")], &[])).unwrap();
        let sidecar = debug_dir.join("app.dwo");
        fs::write(&sidecar, build_object(&[(".debug_line", &[5, 5])], &[])).unwrap();

        let resolved = resolve_debug_artifact(&elf, &resolver(&[debug_dir])).unwrap();
        assert_eq!(resolved.info.source, DebugArtifactSource::UserDir);
        assert_eq!(resolved.info.kind, DebugArtifactKind::SplitDwo);
        assert!(resolved.info.split_dwarf);
    }

    #[test]
    fn keeps_running_when_debuginfod_is_disabled() {
        let dir = temp_dir("debuginfod");
        fs::create_dir_all(&dir).unwrap();
        let elf = dir.join("app.elf");
        fs::write(&elf, build_object(&[], &[])).unwrap();

        let resolved = resolve_debug_artifact(&elf, &resolver(&[])).unwrap();
        assert_eq!(resolved.info.kind, DebugArtifactKind::None);
        assert_eq!(resolved.info.source, DebugArtifactSource::None);
    }

    fn resolver(debug_file_dirs: &[PathBuf]) -> DebugArtifactResolver {
        DebugArtifactResolver {
            debug_file_dirs: debug_file_dirs.to_vec(),
            debuginfod: DebuginfodMode::Off,
            debuginfod_urls: Vec::new(),
            debuginfod_cache_dir: None,
            trace: true,
        }
    }

    fn build_object(sections: &[(&str, &[u8])], _notes: &[(&str, &[u8])]) -> Vec<u8> {
        let mut object = Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);
        for (name, data) in sections {
            let section = object.add_section(Vec::new(), name.as_bytes().to_vec(), SectionKind::Debug);
            object.append_section_data(section, data, 1);
        }
        object.write().unwrap()
    }

    fn build_id_note(build_id: &[u8]) -> Vec<u8> {
        let mut note = Vec::new();
        note.extend_from_slice(&(4u32).to_le_bytes());
        note.extend_from_slice(&(build_id.len() as u32).to_le_bytes());
        note.extend_from_slice(&(3u32).to_le_bytes());
        note.extend_from_slice(b"GNU\0");
        note.extend_from_slice(build_id);
        note
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("fwmap-debug-{label}-{nanos}"))
    }
}
