use std::collections::BTreeMap;

use crate::model::{AnalysisResult, ArchiveContribution, DiffChangeKind, DiffEntry, DiffResult, DiffSummary};

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
        current.object_contributions.iter().map(|item| (object_key(&item.object_path), item.size)),
        previous.object_contributions.iter().map(|item| (object_key(&item.object_path), item.size)),
    );
    let archive_diffs = diff_named(
        current.archive_contributions.iter().map(|item| (archive_member_key(item), item.size)),
        previous.archive_contributions.iter().map(|item| (archive_member_key(item), item.size)),
    );

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
    };

    DiffResult {
        rom_delta: current.memory.rom_bytes as i64 - previous.memory.rom_bytes as i64,
        ram_delta: current.memory.ram_bytes as i64 - previous.memory.ram_bytes as i64,
        summary,
        section_diffs,
        symbol_diffs,
        object_diffs,
        archive_diffs,
    }
}

pub fn section_key(name: &str) -> String {
    name.to_string()
}

pub fn symbol_key(name: &str) -> String {
    name.to_string()
}

pub fn object_key(path: &str) -> String {
    path.to_string()
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
    use super::{archive_member_key, diff_results, names_for_kind, object_key, section_key, symbol_key, top_increases};
    use crate::model::{
        AnalysisResult, ArchiveContribution, BinaryInfo, DebugInfoSummary, DiffChangeKind, MemorySummary,
        ObjectContribution, SectionCategory, SectionTotal, SymbolInfo, ToolchainInfo, ToolchainKind,
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
        assert_eq!(object_key("bar.o"), "bar.o");
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
            toolchain: ToolchainInfo {
                requested: ToolchainSelection::Auto,
                detected: None,
                resolved: ToolchainKind::Gnu,
            },
            debug_info: DebugInfoSummary::default(),
            sections: Vec::new(),
            symbols: symbols
                .iter()
                .map(|(name, size)| SymbolInfo {
                    name: (*name).to_string(),
                    demangled_name: None,
                    section_name: None,
                    object_path: None,
                    size: *size,
                })
                .collect(),
            object_contributions: objects
                .iter()
                .map(|(path, size)| ObjectContribution {
                    object_path: (*path).to_string(),
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
            function_attributions: Vec::new(),
            unknown_source: UnknownSourceBucket::default(),
            warnings: Vec::new(),
        }
    }
}
