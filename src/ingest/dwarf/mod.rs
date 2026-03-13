use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use gimli::{Dwarf, DwarfSections, EndianSlice, LittleEndian, SectionId};
use object::{Object, ObjectSection};

use crate::analyze::AnalyzeOptions;
use crate::model::{
    AddressRange, CompilationUnit, DebugInfoSummary, DwarfMode, FunctionAttribution, LineAttribution, SectionInfo,
    SourceFile, SourceLinesMode, SourceLocation, SourceSpan, UnknownSourceBucket, WarningItem,
};

#[derive(Debug, Clone)]
pub struct DwarfIngestResult {
    pub debug_info: DebugInfoSummary,
    pub compilation_units: Vec<CompilationUnit>,
    pub source_files: Vec<SourceFile>,
    pub line_attributions: Vec<LineAttribution>,
    pub function_attributions: Vec<FunctionAttribution>,
    pub unknown_source: UnknownSourceBucket,
    pub warnings: Vec<WarningItem>,
}

pub fn parse_dwarf(path: &Path, sections: &[SectionInfo], options: &AnalyzeOptions) -> Result<DwarfIngestResult, String> {
    match options.dwarf_mode {
        DwarfMode::Off => Ok(empty_result(options)),
        DwarfMode::Auto | DwarfMode::On => parse_dwarf_enabled(path, sections, options),
    }
}

fn parse_dwarf_enabled(path: &Path, sections: &[SectionInfo], options: &AnalyzeOptions) -> Result<DwarfIngestResult, String> {
    let bytes = fs::read(path).map_err(|err| format!("failed to read ELF for DWARF '{}': {err}", path.display()))?;
    let file = object::File::parse(&*bytes).map_err(|err| format!("failed to parse object for DWARF '{}': {err}", path.display()))?;
    let has_debug_line = file.section_by_name(".debug_line").is_some();
    if !has_debug_line {
        if options.dwarf_mode == DwarfMode::On || options.fail_on_missing_dwarf {
            return Err(format!(
                "DWARF line information was requested but '.debug_line' is missing in '{}'",
                path.display()
            ));
        }
        return Ok(empty_result(options));
    }

    let dwarf_sections =
        DwarfSections::load(|id| load_section(&file, id)).map_err(|err| format!("failed to load DWARF sections: {err}"))?;
    let dwarf = dwarf_sections.borrow(|section| EndianSlice::new(section.as_ref(), LittleEndian));

    let mut result = empty_result(options);
    result.debug_info.dwarf_used = true;

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
    Ok(result)
}

fn empty_result(options: &AnalyzeOptions) -> DwarfIngestResult {
    DwarfIngestResult {
        debug_info: DebugInfoSummary {
            dwarf_mode: options.dwarf_mode,
            source_lines: options.source_lines,
            ..DebugInfoSummary::default()
        },
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
    use super::{aggregate_source_files, normalize_path};
    use crate::analyze::AnalyzeOptions;
    use crate::model::{AddressRange, LineAttribution, SourceLocation, SourceSpan};
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
}
