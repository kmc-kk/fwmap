use std::collections::BTreeMap;

use crate::cpp::aggregate_group_sizes;
use crate::model::{
    AnalysisResult, ArchiveContribution, CppGroupBy, DiffChangeKind, DiffEntry, DiffResult, DiffSummary,
    ObjectSourceKind,
};

pub fn diff_results(current: &AnalysisResult, previous: &AnalysisResult) -> DiffResult {
    let section_diffs = diff_named(
        current.memory.section_totals.iter().map(|item| (section_key(&item.section_name), item.size)),
        previous.memory.section_totals.iter().map(|item| (section_key(&item.section_name), item.size)),
    );
    let symbol_diffs = diff_named(
        current.symbols.iter().map(|item| (symbol_key(&item.name), item.size)),
        previous.symbols.iter().map(|item| (symbol_key(&item.name), item.size)),
    );
    let object_diffs = diff_named(
        current
            .object_contributions
            .iter()
            .map(|item| (object_key(item.source_kind, &item.object_path), item.size)),
        previous
            .object_contributions
            .iter()
            .map(|item| (object_key(item.source_kind, &item.object_path), item.size)),
    );
    let archive_diffs = diff_named(
        current.archive_contributions.iter().map(|item| (archive_member_key(item), item.size)),
        previous.archive_contributions.iter().map(|item| (archive_member_key(item), item.size)),
    );
    let source_file_diffs = diff_named(
        current.source_files.iter().map(|item| (source_file_key(&item.path), item.size)),
        previous.source_files.iter().map(|item| (source_file_key(&item.path), item.size)),
    );
    let function_diffs = diff_named(
        current
            .function_attributions
            .iter()
            .map(|item| (function_key(item.path.as_deref(), &item.raw_name), item.size)),
        previous
            .function_attributions
            .iter()
            .map(|item| (function_key(item.path.as_deref(), &item.raw_name), item.size)),
    );
    let line_diffs = diff_named(
        current
            .line_hotspots
            .iter()
            .map(|item| (line_key(&item.path, item.line_start, item.line_end), item.size)),
        previous
            .line_hotspots
            .iter()
            .map(|item| (line_key(&item.path, item.line_start, item.line_end), item.size)),
    );
    let cpp_template_family_diffs = diff_named(
        aggregate_group_sizes(&current.cpp_view, CppGroupBy::CppTemplateFamily).into_iter(),
        aggregate_group_sizes(&previous.cpp_view, CppGroupBy::CppTemplateFamily).into_iter(),
    );
    let cpp_class_diffs = diff_named(
        aggregate_group_sizes(&current.cpp_view, CppGroupBy::CppClass).into_iter(),
        aggregate_group_sizes(&previous.cpp_view, CppGroupBy::CppClass).into_iter(),
    );
    let cpp_runtime_overhead_diffs = diff_named(
        aggregate_group_sizes(&current.cpp_view, CppGroupBy::CppRuntimeOverhead).into_iter(),
        aggregate_group_sizes(&previous.cpp_view, CppGroupBy::CppRuntimeOverhead).into_iter(),
    );
    let cpp_lambda_group_diffs = diff_named(
        aggregate_group_sizes(&current.cpp_view, CppGroupBy::CppLambdaGroup).into_iter(),
        aggregate_group_sizes(&previous.cpp_view, CppGroupBy::CppLambdaGroup).into_iter(),
    );

    // Keep every diff as the same name/current/previous/delta shape so HTML/JSON/CI can reuse one renderer.
    let summary = DiffSummary {
        section_added: count_kind(&section_diffs, DiffChangeKind::Added),
        section_removed: count_kind(&section_diffs, DiffChangeKind::Removed),
        section_increased: count_kind(&section_diffs, DiffChangeKind::Increased),
        section_decreased: count_kind(&section_diffs, DiffChangeKind::Decreased),
        symbol_added: count_kind(&symbol_diffs, DiffChangeKind::Added),
        symbol_removed: count_kind(&symbol_diffs, DiffChangeKind::Removed),
        symbol_increased: count_kind(&symbol_diffs, DiffChangeKind::Increased),
        symbol_decreased: count_kind(&symbol_diffs, DiffChangeKind::Decreased),
        object_added: count_kind(&object_diffs, DiffChangeKind::Added),
        object_removed: count_kind(&object_diffs, DiffChangeKind::Removed),
        object_increased: count_kind(&object_diffs, DiffChangeKind::Increased),
        object_decreased: count_kind(&object_diffs, DiffChangeKind::Decreased),
        source_file_added: count_kind(&source_file_diffs, DiffChangeKind::Added),
        source_file_removed: count_kind(&source_file_diffs, DiffChangeKind::Removed),
        source_file_increased: count_kind(&source_file_diffs, DiffChangeKind::Increased),
        source_file_decreased: count_kind(&source_file_diffs, DiffChangeKind::Decreased),
        function_added: count_kind(&function_diffs, DiffChangeKind::Added),
        function_removed: count_kind(&function_diffs, DiffChangeKind::Removed),
        function_increased: count_kind(&function_diffs, DiffChangeKind::Increased),
        function_decreased: count_kind(&function_diffs, DiffChangeKind::Decreased),
        line_added: count_kind(&line_diffs, DiffChangeKind::Added),
        line_removed: count_kind(&line_diffs, DiffChangeKind::Removed),
        line_increased: count_kind(&line_diffs, DiffChangeKind::Increased),
        line_decreased: count_kind(&line_diffs, DiffChangeKind::Decreased),
    };

    DiffResult {
        rom_delta: current.memory.rom_bytes as i64 - previous.memory.rom_bytes as i64,
        ram_delta: current.memory.ram_bytes as i64 - previous.memory.ram_bytes as i64,
        unknown_source_delta: current.unknown_source.size as i64 - previous.unknown_source.size as i64,
        summary,
        section_diffs,
        symbol_diffs,
        object_diffs,
        archive_diffs,
        source_file_diffs,
        function_diffs,
        line_diffs,
        cpp_template_family_diffs,
        cpp_class_diffs,
        cpp_runtime_overhead_diffs,
        cpp_lambda_group_diffs,
    }
}

pub fn section_key(name: &str) -> String {
    name.to_string()
}

pub fn symbol_key(name: &str) -> String {
    name.to_string()
}

pub fn object_key(kind: ObjectSourceKind, path: &str) -> String {
    match kind {
        ObjectSourceKind::Object => path.to_string(),
        // Preserve the visible <internal> marker while preventing collisions with a real file path.
        ObjectSourceKind::Internal => format!("[internal] {path}"),
    }
}

pub fn source_file_key(path: &str) -> String {
    path.to_string()
}

pub fn function_key(path: Option<&str>, raw_name: &str) -> String {
    match path {
        Some(path) => format!("{path}::{raw_name}"),
        None => raw_name.to_string(),
    }
}

pub fn line_key(path: &str, line_start: u64, line_end: u64) -> String {
    format!("{path}:{line_start}-{line_end}")
}

pub fn archive_member_key(item: &ArchiveContribution) -> String {
    match &item.member_path {
        Some(member) => format!("{}:{}", item.archive_path, member),
        None => item.archive_path.clone(),
    }
}

pub fn top_increases(entries: &[DiffEntry], limit: usize) -> Vec<DiffEntry> {
    entries
        .iter()
        .filter(|entry| matches!(entry.change, DiffChangeKind::Added | DiffChangeKind::Increased))
        .take(limit)
        .cloned()
        .collect()
}

pub fn names_for_kind(entries: &[DiffEntry], kind: DiffChangeKind, limit: usize) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| entry.change == kind)
        .take(limit)
        .map(|entry| entry.name.clone())
        .collect()
}

fn diff_named(current: impl Iterator<Item = (String, u64)>, previous: impl Iterator<Item = (String, u64)>) -> Vec<DiffEntry> {
    let mut map = BTreeMap::<String, (u64, u64)>::new();
    for (name, size) in current {
        map.entry(name).or_default().0 = size;
    }
    for (name, size) in previous {
        map.entry(name).or_default().1 = size;
    }
    let mut diffs = map
        .into_iter()
        .map(|(name, (current, previous))| {
            let delta = current as i64 - previous as i64;
            let change = if current > 0 && previous == 0 {
                DiffChangeKind::Added
            } else if current == 0 && previous > 0 {
                DiffChangeKind::Removed
            } else if delta > 0 {
                DiffChangeKind::Increased
            } else if delta < 0 {
                DiffChangeKind::Decreased
            } else {
                DiffChangeKind::Unchanged
            };
            DiffEntry {
                name,
                current,
                previous,
                delta,
                change,
            }
        })
        .collect::<Vec<_>>();
    diffs.sort_by(|a, b| {
        b.delta
            .abs()
            .cmp(&a.delta.abs())
            .then_with(|| change_rank(a.change).cmp(&change_rank(b.change)))
            .then_with(|| a.name.cmp(&b.name))
    });
    diffs
}

fn count_kind(entries: &[DiffEntry], kind: DiffChangeKind) -> usize {
    entries.iter().filter(|entry| entry.change == kind).count()
}

fn change_rank(kind: DiffChangeKind) -> usize {
    match kind {
        DiffChangeKind::Added => 0,
        DiffChangeKind::Increased => 1,
        DiffChangeKind::Removed => 2,
        DiffChangeKind::Decreased => 3,
        DiffChangeKind::Unchanged => 4,
        DiffChangeKind::Moved => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        archive_member_key, diff_results, function_key, line_key, names_for_kind, object_key, section_key,
        source_file_key, symbol_key, top_increases,
    };
    use crate::cpp::build_cpp_view;
    use crate::model::{
        AnalysisResult, ArchiveContribution, BinaryInfo, DebugArtifactInfo, DebugInfoSummary, DiffChangeKind,
        FunctionAttribution, LineRangeAttribution, MemorySummary, ObjectContribution, ObjectSourceKind,
        SectionCategory, SectionTotal, SourceFile, SourceSpan, SymbolInfo, ToolchainInfo, ToolchainKind,
        ToolchainSelection, UnknownSourceBucket,
    };

    #[test]
    fn classifies_added_removed_increased_and_decreased() {
        let current = stub_analysis(
            &[(".text", 120), (".new", 8)],
            &[("grow", 10), ("new_symbol", 4)],
            &[("main.o", 30)],
            &[("lib.a", Some("a.o"), 16)],
        );
        let previous = stub_analysis(
            &[(".text", 100), (".old", 8)],
            &[("grow", 8), ("gone", 3)],
            &[("main.o", 10), ("old.o", 3)],
            &[("lib.a", Some("gone.o"), 7)],
        );

        let diff = diff_results(&current, &previous);
        assert!(diff.section_diffs.iter().any(|item| item.name == ".new" && item.change == DiffChangeKind::Added));
        assert!(diff.section_diffs.iter().any(|item| item.name == ".old" && item.change == DiffChangeKind::Removed));
        assert!(diff.symbol_diffs.iter().any(|item| item.name == "grow" && item.change == DiffChangeKind::Increased));
        assert!(diff.object_diffs.iter().any(|item| item.name == "old.o" && item.change == DiffChangeKind::Removed));
        assert_eq!(archive_member_key(&current.archive_contributions[0]), "lib.a:a.o");
    }

    #[test]
    fn exposes_top_growth_and_kind_lists() {
        let current = stub_analysis(&[(".text", 12)], &[("a", 10), ("b", 0)], &[("obj.o", 20)], &[]);
        let previous = stub_analysis(&[(".text", 8)], &[("a", 1), ("c", 5)], &[("obj.o", 2)], &[]);
        let diff = diff_results(&current, &previous);
        assert_eq!(top_increases(&diff.symbol_diffs, 1)[0].name, "a");
        assert!(names_for_kind(&diff.symbol_diffs, DiffChangeKind::Removed, 10).contains(&"c".to_string()));
        assert_eq!(section_key(".text"), ".text");
        assert_eq!(symbol_key("foo"), "foo");
        assert_eq!(object_key(ObjectSourceKind::Object, "bar.o"), "bar.o");
        assert_eq!(object_key(ObjectSourceKind::Internal, "<internal>"), "[internal] <internal>");
        assert_eq!(source_file_key("src/main.c"), "src/main.c");
        assert_eq!(function_key(Some("src/main.c"), "main"), "src/main.c::main");
        assert_eq!(line_key("src/main.c", 10, 12), "src/main.c:10-12");
    }

    #[test]
    fn computes_source_level_diffs() {
        let mut current = stub_analysis(&[], &[], &[], &[]);
        current.source_files = vec![SourceFile {
            path: "src/main.c".to_string(),
            display_path: "src/main.c".to_string(),
            directory: "src".to_string(),
            size: 20,
            functions: 1,
            line_ranges: 2,
        }];
        current.function_attributions = vec![FunctionAttribution {
            raw_name: "main".to_string(),
            demangled_name: None,
            path: Some("src/main.c".to_string()),
            size: 20,
            ranges: vec![SourceSpan {
                path: "src/main.c".to_string(),
                line_start: 10,
                line_end: 12,
                column: None,
            }],
        }];
        current.line_hotspots = vec![LineRangeAttribution {
            path: "src/main.c".to_string(),
            line_start: 10,
            line_end: 12,
            section_name: Some(".text".to_string()),
            size: 20,
        }];
        current.unknown_source.size = 4;
        let mut previous = stub_analysis(&[], &[], &[], &[]);
        previous.source_files = vec![SourceFile {
            path: "src/main.c".to_string(),
            display_path: "src/main.c".to_string(),
            directory: "src".to_string(),
            size: 8,
            functions: 1,
            line_ranges: 1,
        }];
        previous.function_attributions = vec![FunctionAttribution {
            raw_name: "main".to_string(),
            demangled_name: None,
            path: Some("src/main.c".to_string()),
            size: 8,
            ranges: vec![SourceSpan {
                path: "src/main.c".to_string(),
                line_start: 10,
                line_end: 10,
                column: None,
            }],
        }];
        previous.line_hotspots = vec![LineRangeAttribution {
            path: "src/main.c".to_string(),
            line_start: 10,
            line_end: 10,
            section_name: Some(".text".to_string()),
            size: 8,
        }];
        previous.unknown_source.size = 1;
        let diff = diff_results(&current, &previous);
        assert_eq!(diff.unknown_source_delta, 3);
        assert!(diff.source_file_diffs.iter().any(|item| item.name == "src/main.c" && item.delta == 12));
        assert!(diff.function_diffs.iter().any(|item| item.name == "src/main.c::main" && item.delta == 12));
        assert!(diff.line_diffs.iter().any(|item| item.name == "src/main.c:10-12" && item.change == DiffChangeKind::Added));
    }

    #[test]
    fn computes_cpp_group_diffs() {
        let mut current = stub_analysis(&[], &[("_ZN3app3Foo3barEv", 30), ("_ZTVN3app3FooE", 20)], &[], &[]);
        current.symbols[0].demangled_name = Some("app::Foo::bar()".to_string());
        current.symbols[1].demangled_name = Some("vtable for app::Foo".to_string());
        current.cpp_view = build_cpp_view(&current.symbols);

        let mut previous = stub_analysis(&[], &[("_ZN3app3Foo3barEv", 10)], &[], &[]);
        previous.symbols[0].demangled_name = Some("app::Foo::bar()".to_string());
        previous.cpp_view = build_cpp_view(&previous.symbols);

        let diff = diff_results(&current, &previous);
        assert!(diff
            .cpp_class_diffs
            .iter()
            .any(|item| item.name == "app::Foo" && item.delta == 40));
        assert!(diff
            .cpp_runtime_overhead_diffs
            .iter()
            .any(|item| item.name == "vtable" && item.delta == 20));
    }

    fn stub_analysis(
        sections: &[(&str, u64)],
        symbols: &[(&str, u64)],
        objects: &[(&str, u64)],
        archives: &[(&str, Option<&str>, u64)],
    ) -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "a.elf".to_string(),
                arch: "ARM".to_string(),
                elf_class: "ELF32".to_string(),
                endian: "little-endian".to_string(),
            },
            git: None,
            toolchain: ToolchainInfo {
                requested: ToolchainSelection::Auto,
                detected: None,
                resolved: ToolchainKind::Gnu,
                linker_family: crate::model::LinkerFamily::Gnu,
                map_format: crate::model::MapFormat::Unknown,
                parser_warnings_count: 0,
            },
            debug_info: DebugInfoSummary::default(),
            debug_artifact: DebugArtifactInfo::default(),
            policy: None,
            sections: Vec::new(),
            symbols: symbols
                .iter()
                .map(|(name, size)| SymbolInfo {
                    name: (*name).to_string(),
                    demangled_name: None,
                    section_name: None,
                    object_path: None,
                    addr: 0,
                    size: *size,
                })
                .collect(),
            object_contributions: objects
                .iter()
                .map(|(path, size)| ObjectContribution {
                    object_path: (*path).to_string(),
                    source_kind: ObjectSourceKind::Object,
                    section_name: None,
                    size: *size,
                })
                .collect(),
            archive_contributions: archives
                .iter()
                .map(|(archive, member, size)| ArchiveContribution {
                    archive_path: (*archive).to_string(),
                    member_path: member.map(|item| item.to_string()),
                    section_name: None,
                    size: *size,
                })
                .collect(),
            archive_pulls: Vec::new(),
            whole_archive_candidates: Vec::new(),
            relocation_references: Vec::new(),
            cross_references: Vec::new(),
            cpp_view: crate::model::CppView::default(),
            linker_script: None,
            memory: MemorySummary {
                rom_bytes: 0,
                ram_bytes: 0,
                section_totals: sections
                    .iter()
                    .map(|(name, size)| SectionTotal {
                        section_name: (*name).to_string(),
                        size: *size,
                        category: SectionCategory::Rom,
                    })
                    .collect(),
                memory_regions: Vec::new(),
                region_summaries: Vec::new(),
            },
            compilation_units: Vec::new(),
            source_files: Vec::new(),
            line_attributions: Vec::new(),
            line_hotspots: Vec::new(),
            function_attributions: Vec::new(),
            unknown_source: UnknownSourceBucket::default(),
            warnings: Vec::new(),
        }
    }
}
