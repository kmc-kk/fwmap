use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use gimli::{Dwarf, DwarfSections, EndianSlice, LittleEndian, SectionId};
use object::{Object, ObjectSection};

use crate::analyze::AnalyzeOptions;
use crate::debug::ResolvedDebugArtifact;
use crate::model::{
    AddressRange, CompilationUnit, DebugArtifactInfo, DebugArtifactKind, DebugInfoSummary, DwarfMode, FunctionAttribution,
    LineAttribution, SectionInfo, SourceFile, SourceLinesMode, SourceLocation, SourceSpan, UnknownSourceBucket,
    WarningItem, WarningLevel, WarningSource,
};

#[derive(Debug, Clone)]
pub struct DwarfIngestResult {
    pub debug_info: DebugInfoSummary,
    pub debug_artifact: DebugArtifactInfo,
    pub compilation_units: Vec<CompilationUnit>,
    pub source_files: Vec<SourceFile>,
    pub line_attributions: Vec<LineAttribution>,
    pub function_attributions: Vec<FunctionAttribution>,
    pub unknown_source: UnknownSourceBucket,
    pub warnings: Vec<WarningItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DwarfCacheKey {
    path: String,
    modified_nanos: u128,
    len: u64,
    source_root: Option<String>,
    path_remaps: Vec<(String, String)>,
    source_lines: SourceLinesMode,
}

static DWARF_CACHE: OnceLock<Mutex<HashMap<DwarfCacheKey, DwarfIngestResult>>> = OnceLock::new();

pub fn parse_dwarf(
    path: &Path,
    artifact: &ResolvedDebugArtifact,
    sections: &[SectionInfo],
    options: &AnalyzeOptions,
) -> Result<DwarfIngestResult, String> {
    match options.dwarf_mode {
        DwarfMode::Off => Ok(empty_result(options)),
        DwarfMode::Auto | DwarfMode::On => parse_dwarf_enabled(path, artifact, sections, options),
    }
}

fn parse_dwarf_enabled(
    path: &Path,
    artifact: &ResolvedDebugArtifact,
    sections: &[SectionInfo],
    options: &AnalyzeOptions,
) -> Result<DwarfIngestResult, String> {
    let artifact_path = artifact.info.path.as_deref().unwrap_or_else(|| path.to_str().unwrap_or_default());
    let artifact_path = Path::new(artifact_path);
    if let Some(cache_key) = dwarf_cache_key(artifact_path, options) {
        if let Some(cached) = lookup_cache(&cache_key)? {
            return Ok(cached);
        }
    }

    let bytes = artifact.bytes.clone();
    let file = object::File::parse(&*bytes).map_err(|err| format!("failed to parse object for DWARF '{}': {err}", path.display()))?;
    let split_dwarf = detect_split_dwarf(&file);
    let has_debug_line = file.section_by_name(".debug_line").is_some();
    let resolved_external_debug = artifact.info.kind != DebugArtifactKind::None;
    if let Some(kind) = split_dwarf.as_deref() {
        if !has_debug_line {
            let message = format!(
                "split DWARF ({kind}) was detected in '{}' but no usable line information was available in the resolved debug artifact",
                path.display()
            );
            if options.dwarf_mode == DwarfMode::On || options.fail_on_missing_dwarf {
                return Err(message);
            }
            let mut result = empty_result(options);
            result.debug_info.split_dwarf_detected = true;
            result.debug_info.split_dwarf_kind = Some(kind.to_string());
            result.debug_artifact = artifact.info.clone();
            result.warnings.push(WarningItem {
                level: WarningLevel::Info,
                code: "SPLIT_DWARF_UNSUPPORTED".to_string(),
                message,
                source: WarningSource::Elf,
                related: Some(path.display().to_string()),
            });
            return Ok(result);
        }
    }
    if !has_debug_line {
        if options.dwarf_mode == DwarfMode::On || options.fail_on_missing_dwarf {
            return Err(format!(
                "DWARF line information was requested but '.debug_line' is missing in '{}'",
                path.display()
            ));
        }
        let mut result = empty_result(options);
        result.debug_artifact = artifact.info.clone();
        if !resolved_external_debug {
            result.warnings.push(WarningItem {
                level: WarningLevel::Info,
                code: "DEBUG_ARTIFACT_NOT_FOUND".to_string(),
                message: format!("No usable debug artifact was found for '{}'", path.display()),
                source: WarningSource::Elf,
                related: Some(path.display().to_string()),
            });
        }
        return Ok(result);
    }

    let dwarf_sections =
        DwarfSections::load(|id| load_section(&file, id)).map_err(|err| format!("failed to load DWARF sections: {err}"))?;
    let dwarf = dwarf_sections.borrow(|section| EndianSlice::new(section.as_ref(), LittleEndian));

    let mut result = empty_result(options);
    result.debug_artifact = artifact.info.clone();
    result.debug_info.dwarf_used = true;
    result.debug_info.split_dwarf_detected = artifact.info.split_dwarf || split_dwarf.is_some();
    result.debug_info.split_dwarf_kind = split_dwarf
        .clone()
        .or_else(|| match artifact.info.kind {
            DebugArtifactKind::SplitDwo => Some("dwo".to_string()),
            DebugArtifactKind::SplitDwp => Some("dwp".to_string()),
            _ => None,
        });
    if let Some(kind) = split_dwarf {
        result.warnings.push(WarningItem {
            level: WarningLevel::Info,
            code: "SPLIT_DWARF_PARTIAL".to_string(),
            message: format!(
                "split DWARF marker ({kind}) was detected; attribution uses the resolved debug artifact sections that are currently available"
            ),
            source: WarningSource::Elf,
            related: Some(path.display().to_string()),
        });
    }

    let mut units = dwarf.units();
    while let Some(header) = units.next().map_err(|err| format!("failed to iterate DWARF units: {err}"))? {
        let unit = dwarf.unit(header).map_err(|err| format!("failed to parse DWARF unit: {err}"))?;
        let comp_dir = unit
            .comp_dir
            .as_ref()
            .map(|dir| dir.to_string_lossy().into_owned());
        result.compilation_units.push(CompilationUnit {
            name: None,
            comp_dir: comp_dir.clone(),
            file_count: unit.line_program.as_ref().map(|program| program.header().file_names().len()).unwrap_or(0),
        });
        result.debug_info.compilation_units += 1;

        let Some(program) = unit.line_program.clone() else {
            continue;
        };
        let (program, sequences) = program.sequences().map_err(|err| format!("failed to decode DWARF line sequences: {err}"))?;
        for sequence in sequences {
            let mut rows = program.resume_from(&sequence);
            let mut previous: Option<(u64, String, u64, Option<u64>)> = None;
            while let Some((header, row)) = rows.next_row().map_err(|err| format!("failed to decode DWARF line row: {err}"))?
            {
                let current_address = row.address();
                if let Some((prev_address, prev_path, prev_line, prev_column)) = previous.take() {
                    if current_address > prev_address {
                        let range = AddressRange {
                            start: prev_address,
                            end: current_address,
                            section_name: section_name_for_range(sections, prev_address, current_address),
                        };
                        let size = current_address - prev_address;
                        if prev_line > 0 {
                            result.line_attributions.push(LineAttribution {
                                location: SourceLocation {
                                    path: prev_path.clone(),
                                    line: prev_line,
                                    column: prev_column,
                                },
                                span: SourceSpan {
                                    path: prev_path.clone(),
                                    line_start: prev_line,
                                    line_end: prev_line,
                                    column: prev_column,
                                },
                                range,
                                size,
                            });
                        } else {
                            // Optimized builds often emit line 0 or compiler-generated gaps. Track
                            // them explicitly so unknown attribution is explainable instead of silent.
                            result.debug_info.line_zero_ranges += 1;
                            result.debug_info.generated_ranges += 1;
                            result.unknown_source.size += size;
                            result.unknown_source.ranges.push(range);
                        }
                    }
                }
                if row.end_sequence() {
                    continue;
                }
                let path = resolve_row_path(&dwarf, &unit, header, row.file(header), comp_dir.as_deref(), options)?;
                let line = row.line().map(|line| line.get()).unwrap_or(0);
                let column = match row.column() {
                    gimli::ColumnType::LeftEdge => None,
                    gimli::ColumnType::Column(value) => Some(value.get()),
                };
                if path.is_none() {
                    result.debug_info.generated_ranges += 1;
                }
                previous = path.map(|path| (current_address, path, line, column));
            }
        }
    }

    result.source_files = aggregate_source_files(&result.line_attributions);
    let code_bytes = code_section_bytes(sections);
    let attributed_bytes = result.line_attributions.iter().map(|item| item.size).sum::<u64>();
    if code_bytes > 0 {
        result.unknown_source.size += code_bytes.saturating_sub(attributed_bytes);
        result.debug_info.unknown_source_ratio = result.unknown_source.size as f64 / code_bytes as f64;
    }
    if matches!(options.source_lines, SourceLinesMode::Off) {
        result.source_files.clear();
        result.line_attributions.clear();
    }
    if result.debug_info.line_zero_ranges > 0 {
        result.warnings.push(WarningItem {
            level: WarningLevel::Info,
            code: "DWARF_LINE_ZERO".to_string(),
            message: format!(
                "DWARF emitted {} line-0 ranges; these bytes were counted as unknown source",
                result.debug_info.line_zero_ranges
            ),
            source: WarningSource::Elf,
            related: Some(path.display().to_string()),
        });
    }
    if let Some(cache_key) = dwarf_cache_key(artifact_path, options) {
        store_cache(cache_key, &result)?;
    }
    Ok(result)
}

fn empty_result(options: &AnalyzeOptions) -> DwarfIngestResult {
    DwarfIngestResult {
        debug_info: DebugInfoSummary {
            dwarf_mode: options.dwarf_mode,
            source_lines: options.source_lines,
            ..DebugInfoSummary::default()
        },
        debug_artifact: DebugArtifactInfo::default(),
        compilation_units: Vec::new(),
        source_files: Vec::new(),
        line_attributions: Vec::new(),
        function_attributions: Vec::new(),
        unknown_source: UnknownSourceBucket::default(),
        warnings: Vec::new(),
    }
}

fn load_section<'a>(file: &'a object::File<'a>, id: SectionId) -> Result<Cow<'a, [u8]>, gimli::Error> {
    Ok(file
        .section_by_name(id.name())
        .and_then(|section| section.uncompressed_data().ok())
        .unwrap_or(Cow::Borrowed(&[])))
}

fn dwarf_cache_key(path: &Path, options: &AnalyzeOptions) -> Option<DwarfCacheKey> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    Some(DwarfCacheKey {
        path: path.display().to_string(),
        modified_nanos: modified,
        len: metadata.len(),
        source_root: options.source_root.as_ref().map(|item| item.to_string_lossy().into_owned()),
        path_remaps: options.path_remaps.clone(),
        source_lines: options.source_lines,
    })
}

fn lookup_cache(key: &DwarfCacheKey) -> Result<Option<DwarfIngestResult>, String> {
    let cache = DWARF_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cached = cache
        .lock()
        .map_err(|_| "failed to lock DWARF cache".to_string())?
        .get(key)
        .cloned();
    if let Some(result) = cached.as_mut() {
        result.debug_info.cache_hit = true;
    }
    Ok(cached)
}

fn store_cache(key: DwarfCacheKey, result: &DwarfIngestResult) -> Result<(), String> {
    let cache = DWARF_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    cache
        .lock()
        .map_err(|_| "failed to lock DWARF cache".to_string())?
        .insert(key, result.clone());
    Ok(())
}

fn detect_split_dwarf(file: &object::File<'_>) -> Option<String> {
    const SPLIT_SECTIONS: [(&str, &str); 7] = [
        (".debug_info.dwo", ".dwo sections"),
        (".debug_abbrev.dwo", ".dwo sections"),
        (".debug_str.dwo", ".dwo sections"),
        (".debug_line.dwo", ".dwo sections"),
        (".gnu_debugaltlink", "gnu debug altlink"),
        (".debug_cu_index", ".dwp index"),
        (".debug_tu_index", ".dwp index"),
    ];
    SPLIT_SECTIONS
        .iter()
        .find(|(name, _)| file.section_by_name(name).is_some())
        .map(|(_, kind)| (*kind).to_string())
}

fn resolve_row_path<R: gimli::Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    unit: &gimli::Unit<R>,
    header: &gimli::LineProgramHeader<R>,
    file: Option<&gimli::FileEntry<R, usize>>,
    comp_dir: Option<&str>,
    options: &AnalyzeOptions,
) -> Result<Option<String>, String> {
    let Some(file) = file else {
        return Ok(None);
    };
    let file_name = dwarf
        .attr_string(unit, file.path_name())
        .map_err(|err| format!("failed to read DWARF file path: {err}"))?
        .to_string_lossy()
        .map_err(|_| "failed to decode DWARF file path".to_string())?
        .into_owned();
    let dir_name = file
        .directory(header)
        .and_then(|dir| dwarf.attr_string(unit, dir).ok())
        .and_then(|dir| dir.to_string_lossy().ok().map(|value| value.into_owned()))
        .or_else(|| comp_dir.map(str::to_string));
    Ok(Some(normalize_path(dir_name.as_deref(), &file_name, options)))
}

fn normalize_path(directory: Option<&str>, file_name: &str, options: &AnalyzeOptions) -> String {
    let mut path = match directory {
        Some(dir) if !Path::new(file_name).is_absolute() && !dir.is_empty() => format!("{dir}/{file_name}"),
        _ => file_name.to_string(),
    };
    path = path.replace('\\', "/");
    for (from, to) in &options.path_remaps {
        let from_norm = from.replace('\\', "/");
        if path.starts_with(&from_norm) {
            path = format!("{}{}", to.replace('\\', "/"), &path[from_norm.len()..]);
        }
    }
    if let Some(root) = options.source_root.as_ref() {
        if !Path::new(&path).is_absolute() {
            path = root.join(path).to_string_lossy().replace('\\', "/");
        }
    }
    path
}

fn aggregate_source_files(lines: &[LineAttribution]) -> Vec<SourceFile> {
    let mut totals = BTreeMap::<String, (u64, BTreeSet<(u64, u64)>)>::new();
    for line in lines {
        let entry = totals.entry(line.location.path.clone()).or_insert_with(|| (0, BTreeSet::new()));
        entry.0 += line.size;
        entry.1.insert((line.span.line_start, line.span.line_end));
    }
    let mut files = totals
        .into_iter()
        .map(|(path, (size, ranges))| SourceFile {
            directory: Path::new(&path)
                .parent()
                .map(|item| item.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default(),
            display_path: path.clone(),
            path,
            size,
            functions: 0,
            line_ranges: ranges.len(),
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    files
}

fn section_name_for_range(sections: &[SectionInfo], start: u64, end: u64) -> Option<String> {
    sections
        .iter()
        .find(|section| start >= section.addr && end <= section.addr.saturating_add(section.size))
        .map(|section| section.name.clone())
}

fn code_section_bytes(sections: &[SectionInfo]) -> u64 {
    sections
        .iter()
        .filter(|section| section.flags.iter().any(|flag| flag == "EXEC") || section.name.starts_with(".text"))
        .map(|section| section.size)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{aggregate_source_files, detect_split_dwarf, normalize_path};
    use crate::analyze::AnalyzeOptions;
    use crate::model::{AddressRange, LineAttribution, SourceLocation, SourceSpan};
    use object::write::{Object, StandardSection, Symbol, SymbolSection};
    use object::{Architecture, BinaryFormat, Endianness, SectionKind, SymbolFlags, SymbolKind, SymbolScope};
    use std::path::PathBuf;

    #[test]
    fn path_normalization_applies_root_and_remap() {
        let options = AnalyzeOptions {
            source_root: Some(PathBuf::from("/repo")),
            path_remaps: vec![("C:/work".to_string(), "/src".to_string())],
            ..AnalyzeOptions::default()
        };
        let path = normalize_path(Some("C:\\work\\project\\src"), "main.c", &options);
        assert!(path.contains("/src/project/src/main.c"));
    }

    #[test]
    fn aggregates_source_files_by_path() {
        let files = aggregate_source_files(&[
            LineAttribution {
                location: SourceLocation {
                    path: "src/main.c".to_string(),
                    line: 10,
                    column: None,
                },
                span: SourceSpan {
                    path: "src/main.c".to_string(),
                    line_start: 10,
                    line_end: 10,
                    column: None,
                },
                range: AddressRange {
                    start: 0x1000,
                    end: 0x1004,
                    section_name: Some(".text".to_string()),
                },
                size: 4,
            },
            LineAttribution {
                location: SourceLocation {
                    path: "src/main.c".to_string(),
                    line: 11,
                    column: None,
                },
                span: SourceSpan {
                    path: "src/main.c".to_string(),
                    line_start: 11,
                    line_end: 11,
                    column: None,
                },
                range: AddressRange {
                    start: 0x1004,
                    end: 0x1008,
                    section_name: Some(".text".to_string()),
                },
                size: 4,
            },
        ]);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].size, 8);
        assert_eq!(files[0].line_ranges, 2);
    }

    #[test]
    fn detects_split_dwarf_sections() {
        let mut object = Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);
        let section = object.add_section(Vec::new(), b".debug_info.dwo".to_vec(), SectionKind::Debug);
        object.append_section_data(section, &[1, 2, 3], 1);
        let text = object.section_id(StandardSection::Text);
        object.append_section_data(text, &[0x90], 1);
        object.add_symbol(Symbol {
            name: b"main".to_vec(),
            value: 0,
            size: 1,
            kind: SymbolKind::Text,
            scope: SymbolScope::Compilation,
            weak: false,
            section: SymbolSection::Section(text),
            flags: SymbolFlags::None,
        });
        let bytes = object.write().unwrap();
        let parsed = object::File::parse(&*bytes).unwrap();
        assert_eq!(detect_split_dwarf(&parsed), Some(".dwo sections".to_string()));
    }
}
