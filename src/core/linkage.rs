use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Serialize;

use crate::diff::top_increases;
use crate::model::{
    AnalysisResult, DiffResult, LinkerScriptInfo, ObjectSourceKind, SectionPlacement,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkageGraph {
    pub nodes: Vec<LinkageNode>,
    pub edges: Vec<LinkageEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkageNode {
    pub id: String,
    pub kind: LinkageNodeKind,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkageNodeKind {
    Archive,
    Object,
    Section,
    Symbol,
    Region,
    EntryRoot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkageEdge {
    pub from: String,
    pub to: String,
    pub kind: LinkageEdgeKind,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkageEdgeKind {
    Reference,
    Resolution,
    ArchivePull,
    ScriptPlacement,
    EntryRoot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExplainResult {
    pub target: String,
    pub kind: ExplainTargetKind,
    pub summary: String,
    pub confidence: Confidence,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplainTargetKind {
    Symbol,
    Object,
    Section,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub detail: String,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    MapContribution,
    ArchivePull,
    SymbolPlacement,
    ScriptPlacement,
    EntryRoot,
    CandidateReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Confidence::Low => f.write_str("low"),
            Confidence::Medium => f.write_str("medium"),
            Confidence::High => f.write_str("high"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct WhyLinkedCollection {
    pub top_symbols: Vec<ExplainResult>,
    pub top_objects: Vec<ExplainResult>,
}

pub fn build_linkage_graph(current: &AnalysisResult) -> LinkageGraph {
    let mut nodes = BTreeMap::<String, LinkageNode>::new();
    let mut edges = BTreeSet::<(String, String, LinkageEdgeKind, Option<String>)>::new();

    for section in &current.sections {
        let id = format!("section:{}", section.name);
        nodes.entry(id.clone()).or_insert(LinkageNode {
            id: id.clone(),
            kind: LinkageNodeKind::Section,
            name: section.name.clone(),
        });
    }

    for symbol in &current.symbols {
        let symbol_id = format!("symbol:{}", symbol.name);
        nodes.entry(symbol_id.clone()).or_insert(LinkageNode {
            id: symbol_id.clone(),
            kind: LinkageNodeKind::Symbol,
            name: symbol.name.clone(),
        });
        if let Some(section_name) = symbol.section_name.as_deref() {
            edges.insert((
                symbol_id,
                format!("section:{section_name}"),
                LinkageEdgeKind::Resolution,
                Some(format!("symbol {} resolves into {}", symbol.name, section_name)),
            ));
        }
        for cref in current.cross_references.iter().filter(|item| item.symbol == symbol.name) {
            let target_id = format!("symbol:{}", cref.symbol);
            let provider_id = object_node_id(&cref.defined_in, ObjectSourceKind::Object);
            nodes.entry(provider_id.clone()).or_insert(LinkageNode {
                id: provider_id.clone(),
                kind: LinkageNodeKind::Object,
                name: cref.defined_in.clone(),
            });
            edges.insert((
                provider_id,
                target_id.clone(),
                LinkageEdgeKind::Resolution,
                Some(format!("{} defines {}", cref.defined_in, cref.symbol)),
            ));
            for reference in &cref.referenced_by {
                let ref_id = object_node_id(reference, ObjectSourceKind::Object);
                nodes.entry(ref_id.clone()).or_insert(LinkageNode {
                    id: ref_id.clone(),
                    kind: LinkageNodeKind::Object,
                    name: reference.clone(),
                });
                edges.insert((
                    ref_id,
                    target_id.clone(),
                    LinkageEdgeKind::Reference,
                    Some(format!("{} references {}", reference, cref.symbol)),
                ));
            }
        }
        for pull in current.archive_pulls.iter().filter(|item| item.symbol == symbol.name) {
            let ref_id = object_node_id(&pull.referenced_by, ObjectSourceKind::Object);
            nodes.entry(ref_id.clone()).or_insert(LinkageNode {
                id: ref_id.clone(),
                kind: LinkageNodeKind::Object,
                name: pull.referenced_by.clone(),
            });
            edges.insert((
                ref_id,
                format!("symbol:{}", pull.symbol),
                LinkageEdgeKind::Reference,
                Some(format!(
                    "{} pulls {} to resolve {}",
                    pull.referenced_by, pull.archive_member, pull.symbol
                )),
            ));
        }
        if is_entry_symbol(&symbol.name) {
            let entry_id = format!("entry:{}", symbol.name);
            nodes.entry(entry_id.clone()).or_insert(LinkageNode {
                id: entry_id.clone(),
                kind: LinkageNodeKind::EntryRoot,
                name: symbol.name.clone(),
            });
            edges.insert((
                entry_id,
                format!("symbol:{}", symbol.name),
                LinkageEdgeKind::EntryRoot,
                Some("heuristic entry/root symbol".to_string()),
            ));
        }
    }

    for object in &current.object_contributions {
        let object_id = object_node_id(&object.object_path, object.source_kind);
        nodes.entry(object_id.clone()).or_insert(LinkageNode {
            id: object_id.clone(),
            kind: LinkageNodeKind::Object,
            name: object.object_path.clone(),
        });
        if let Some(section_name) = object.section_name.as_deref() {
            edges.insert((
                object_id,
                format!("section:{section_name}"),
                LinkageEdgeKind::Resolution,
                Some(format!("{} contributes {} bytes", object.object_path, object.size)),
            ));
        }
    }

    for archive in &current.archive_contributions {
        let archive_id = format!("archive:{}", archive.archive_path);
        nodes.entry(archive_id.clone()).or_insert(LinkageNode {
            id: archive_id.clone(),
            kind: LinkageNodeKind::Archive,
            name: archive.archive_path.clone(),
        });
        if let Some(member) = archive.member_path.as_deref() {
            let object_name = format!("{}({member})", archive.archive_path);
            let object_id = object_node_id(&object_name, ObjectSourceKind::Object);
            nodes.entry(object_id.clone()).or_insert(LinkageNode {
                id: object_id.clone(),
                kind: LinkageNodeKind::Object,
                name: object_name.clone(),
            });
            edges.insert((
                archive_id.clone(),
                object_id.clone(),
                LinkageEdgeKind::ArchivePull,
                Some(format!("{member} contributes {} bytes", archive.size)),
            ));
            if let Some(section_name) = archive.section_name.as_deref() {
                edges.insert((
                    object_id,
                    format!("section:{section_name}"),
                    LinkageEdgeKind::Resolution,
                    Some(format!("{object_name} contributes via archive member")),
                ));
            }
        }
    }

    if let Some(lds) = current.linker_script.as_ref() {
        for placement in &lds.placements {
            let region_id = format!("region:{}", placement.region_name);
            nodes.entry(region_id.clone()).or_insert(LinkageNode {
                id: region_id.clone(),
                kind: LinkageNodeKind::Region,
                name: placement.region_name.clone(),
            });
            edges.insert((
                format!("section:{}", placement.section_name),
                region_id,
                LinkageEdgeKind::ScriptPlacement,
                Some(script_detail(placement)),
            ));
        }
    }

    LinkageGraph {
        nodes: nodes.into_values().collect(),
        edges: edges
            .into_iter()
            .map(|(from, to, kind, detail)| LinkageEdge { from, to, kind, detail })
            .collect(),
    }
}

pub fn explain_symbol(current: &AnalysisResult, symbol_name: &str) -> Option<ExplainResult> {
    let symbol = current.symbols.iter().find(|item| {
        item.name == symbol_name || item.demangled_name.as_deref() == Some(symbol_name)
    })?;
    let mut evidence = vec![Evidence {
        kind: EvidenceKind::SymbolPlacement,
        detail: format!(
            "Symbol {} is present in {} at 0x{:x} ({} bytes)",
            symbol.name,
            symbol.section_name.as_deref().unwrap_or("<unknown section>"),
            symbol.addr,
            symbol.size
        ),
        source: "elf".to_string(),
    }];

    let mut confidence = Confidence::Medium;
    let mut summary = format!(
        "{} is linked because it appears in the final ELF symbol table",
        symbol.demangled_name.as_deref().unwrap_or(&symbol.name)
    );

    if let Some(section_name) = symbol.section_name.as_deref() {
        let section_objects = current
            .object_contributions
            .iter()
            .filter(|item| item.section_name.as_deref() == Some(section_name))
            .collect::<Vec<_>>();
        if let Some(object_path) = symbol.object_path.as_deref() {
            evidence.push(Evidence {
                kind: EvidenceKind::MapContribution,
                detail: format!("ELF symbol metadata points to object {}", object_path),
                source: "elf".to_string(),
            });
            summary = format!(
                "{} is linked through {} and resolves into {}",
                symbol.demangled_name.as_deref().unwrap_or(&symbol.name),
                object_path,
                section_name
            );
            confidence = Confidence::High;
        } else if let Some(primary) = section_objects.first() {
            evidence.push(Evidence {
                kind: EvidenceKind::CandidateReference,
                detail: format!(
                    "Section {} receives {} bytes from candidate object {}",
                    section_name, primary.size, primary.object_path
                ),
                source: "map".to_string(),
            });
            summary = format!(
                "{} resolves into {}. Candidate contributing object: {}",
                symbol.demangled_name.as_deref().unwrap_or(&symbol.name),
                section_name,
                primary.object_path
            );
            confidence = Confidence::Low;
        }
        if let Some(cref) = current.cross_references.iter().find(|item| item.symbol == symbol.name) {
            for reference in &cref.referenced_by {
                evidence.push(Evidence {
                    kind: EvidenceKind::CandidateReference,
                    detail: format!("Cross reference table shows {} referenced by {}", cref.symbol, reference),
                    source: "map.cref".to_string(),
                });
            }
            if !cref.referenced_by.is_empty() {
                summary = format!(
                    "{} is linked because {} is referenced by {}",
                    symbol.demangled_name.as_deref().unwrap_or(&symbol.name),
                    cref.symbol,
                    cref.referenced_by.join(", ")
                );
                confidence = confidence.max(Confidence::High);
            }
        }
        for pull in current.archive_pulls.iter().filter(|item| item.symbol == symbol.name) {
            evidence.push(Evidence {
                kind: EvidenceKind::ArchivePull,
                detail: format!(
                    "Archive member {} was pulled because {} referenced {}",
                    pull.archive_member, pull.referenced_by, pull.symbol
                ),
                source: "map.archive_pull".to_string(),
            });
            summary = format!(
                "{} is linked because {} referenced {} and pulled {}",
                symbol.demangled_name.as_deref().unwrap_or(&symbol.name),
                pull.referenced_by,
                pull.symbol,
                pull.archive_member
            );
            confidence = confidence.max(Confidence::High);
        }
        if let Some(placement) = placement_for_section(current.linker_script.as_ref(), section_name) {
            evidence.push(script_evidence(placement));
            if placement.keep {
                confidence = confidence.max(Confidence::Medium);
            }
        }
    }

    if is_entry_symbol(&symbol.name) {
        evidence.push(Evidence {
            kind: EvidenceKind::EntryRoot,
            detail: format!("{} matches a common entry/root symbol heuristic", symbol.name),
            source: "heuristic".to_string(),
        });
        confidence = Confidence::High;
    }

    Some(ExplainResult {
        target: symbol_name.to_string(),
        kind: ExplainTargetKind::Symbol,
        summary,
        confidence,
        evidence,
    })
}

pub fn explain_object(current: &AnalysisResult, query: &str) -> Option<ExplainResult> {
    let normalized = normalize_object_query(query);
    let mut evidence = Vec::new();
    let mut confidence = Confidence::Low;
    let archive_hits = current
        .archive_contributions
        .iter()
        .filter(|item| normalize_archive_member(item) == normalized)
        .collect::<Vec<_>>();
    if !archive_hits.is_empty() {
        let pull_hits = current
            .archive_pulls
            .iter()
            .filter(|item| normalize_object_query(&item.archive_member) == normalized)
            .collect::<Vec<_>>();
        let normalized_defined = normalize_object_query(query);
        let cref_hits = current
            .cross_references
            .iter()
            .filter(|item| normalize_object_query(&item.defined_in) == normalized_defined)
            .collect::<Vec<_>>();
        let sections = archive_hits
            .iter()
            .map(|item| format!("{} ({})", item.section_name.clone().unwrap_or_else(|| "<unknown>".to_string()), item.size))
            .collect::<Vec<_>>();
        let mut summary = format!(
            "{} is linked because the map records archive member contributions to {}",
            query,
            sections.join(", ")
        );
        for hit in &archive_hits {
            evidence.push(Evidence {
                kind: EvidenceKind::ArchivePull,
                detail: format!(
                    "Archive {} member {} contributes {} bytes to {}",
                    hit.archive_path,
                    hit.member_path.as_deref().unwrap_or("<unknown>"),
                    hit.size,
                    hit.section_name.as_deref().unwrap_or("<unknown section>")
                ),
                source: "map".to_string(),
            });
            if let Some(section_name) = hit.section_name.as_deref() {
                if let Some(placement) = placement_for_section(current.linker_script.as_ref(), section_name) {
                    evidence.push(script_evidence(placement));
                    if placement.keep {
                        confidence = Confidence::Medium;
                    }
                }
            }
        }
        for pull in &pull_hits {
            evidence.push(Evidence {
                kind: EvidenceKind::ArchivePull,
                detail: format!(
                    "Archive pull table shows {} included because {} referenced {}",
                    pull.archive_member, pull.referenced_by, pull.symbol
                ),
                source: "map.archive_pull".to_string(),
            });
        }
        for cref in &cref_hits {
            for reference in &cref.referenced_by {
                evidence.push(Evidence {
                    kind: EvidenceKind::CandidateReference,
                    detail: format!("Cross reference table shows {} referenced by {}", cref.symbol, reference),
                    source: "map.cref".to_string(),
                });
            }
        }
        if !pull_hits.is_empty() {
            confidence = Confidence::High;
            summary = format!(
                "{} is linked to satisfy {}",
                query,
                pull_hits
                    .iter()
                    .map(|item| format!("{} from {}", item.symbol, item.referenced_by))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        } else if cref_hits.iter().any(|item| !item.referenced_by.is_empty()) {
            confidence = Confidence::High;
            summary = format!(
                "{} is linked to satisfy {}",
                query,
                cref_hits
                    .iter()
                    .flat_map(|item| item.referenced_by.iter().map(move |reference| format!("{} from {}", item.symbol, reference)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        confidence = confidence.max(Confidence::Medium);
        return Some(ExplainResult {
            target: query.to_string(),
            kind: ExplainTargetKind::Object,
            summary,
            confidence,
            evidence,
        });
    }

    let object_hits = current
        .object_contributions
        .iter()
        .filter(|item| normalize_object_query(&item.object_path) == normalized)
        .collect::<Vec<_>>();
    if object_hits.is_empty() {
        return None;
    }
    let mut summary = format!(
        "{} is linked because the map records direct contributions into the final image",
        query
    );
    for hit in &object_hits {
        evidence.push(Evidence {
            kind: EvidenceKind::MapContribution,
            detail: format!(
                "{} contributes {} bytes to {}",
                hit.object_path,
                hit.size,
                hit.section_name.as_deref().unwrap_or("<unknown section>")
            ),
            source: "map".to_string(),
        });
        if let Some(section_name) = hit.section_name.as_deref() {
            if let Some(placement) = placement_for_section(current.linker_script.as_ref(), section_name) {
                evidence.push(script_evidence(placement));
                if placement.keep {
                    summary = format!(
                        "{} is retained by linker placement and contributes directly to the output image",
                        query
                    );
                }
            }
        }
    }
    for pull in current
        .archive_pulls
        .iter()
        .filter(|item| normalize_object_query(&item.referenced_by) == normalized)
    {
        evidence.push(Evidence {
            kind: EvidenceKind::CandidateReference,
            detail: format!(
                "Archive pull table shows {} referenced {} and pulled {}",
                pull.referenced_by, pull.symbol, pull.archive_member
            ),
            source: "map.archive_pull".to_string(),
        });
        summary = format!(
            "{} is linked because it references {} and pulls {}",
            query, pull.symbol, pull.archive_member
        );
        confidence = Confidence::High;
    }
    let cref_hits = current
        .cross_references
        .iter()
        .filter(|item| normalize_object_query(&item.defined_in) == normalized)
        .collect::<Vec<_>>();
    for cref in &cref_hits {
        for reference in &cref.referenced_by {
            evidence.push(Evidence {
                kind: EvidenceKind::CandidateReference,
                detail: format!("Cross reference table shows {} referenced by {}", cref.symbol, reference),
                source: "map.cref".to_string(),
            });
        }
    }
    if cref_hits.iter().any(|item| !item.referenced_by.is_empty()) {
        confidence = Confidence::High;
        summary = format!(
            "{} is linked to resolve {}",
            query,
            cref_hits
                .iter()
                .flat_map(|item| item.referenced_by.iter().map(move |reference| format!("{} from {}", item.symbol, reference)))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    Some(ExplainResult {
        target: query.to_string(),
        kind: ExplainTargetKind::Object,
        summary,
        confidence: if confidence != Confidence::Low {
            confidence
        } else if object_hits.iter().any(|item| item.section_name.is_some()) {
            Confidence::Medium
        } else {
            Confidence::Low
        },
        evidence,
    })
}

pub fn explain_section(current: &AnalysisResult, section_name: &str) -> Option<ExplainResult> {
    let section = current.sections.iter().find(|item| item.name == section_name)?;
    let placement = placement_for_section(current.linker_script.as_ref(), section_name);
    let mut evidence = vec![Evidence {
        kind: EvidenceKind::MapContribution,
        detail: format!("Section {} is present at 0x{:x} with {} bytes", section.name, section.addr, section.size),
        source: "elf".to_string(),
    }];
    let mut summary = format!("Section {} is linked because it exists in the final image", section_name);
    let mut confidence = Confidence::Low;

    if let Some(placement) = placement {
        evidence.push(script_evidence(placement));
        summary = format!(
            "Section {} is placed in {} by the linker script",
            section_name, placement.region_name
        );
        confidence = if placement.keep { Confidence::High } else { Confidence::Medium };
    }

    Some(ExplainResult {
        target: section_name.to_string(),
        kind: ExplainTargetKind::Section,
        summary,
        confidence,
        evidence,
    })
}

pub fn explain_top_growth(current: &AnalysisResult, diff: &DiffResult, limit: usize) -> WhyLinkedCollection {
    let limit = limit.max(1);
    let top_symbols = top_increases(&diff.symbol_diffs, limit)
        .into_iter()
        .filter_map(|entry| explain_symbol(current, &entry.name))
        .collect();
    let top_objects = top_increases(&diff.object_diffs, limit)
        .into_iter()
        .filter_map(|entry| explain_object(current, &entry.name))
        .collect();
    WhyLinkedCollection { top_symbols, top_objects }
}

fn placement_for_section<'a>(lds: Option<&'a LinkerScriptInfo>, section_name: &str) -> Option<&'a SectionPlacement> {
    lds?.placements.iter().find(|item| item.section_name == section_name)
}

fn script_evidence(placement: &SectionPlacement) -> Evidence {
    Evidence {
        kind: EvidenceKind::ScriptPlacement,
        detail: script_detail(placement),
        source: "linker_script".to_string(),
    }
}

fn script_detail(placement: &SectionPlacement) -> String {
    if placement.keep {
        format!(
            "Linker script places {} in {} and marks it with KEEP",
            placement.section_name, placement.region_name
        )
    } else {
        format!(
            "Linker script places {} in {}",
            placement.section_name, placement.region_name
        )
    }
}

fn is_entry_symbol(name: &str) -> bool {
    matches!(name, "_start" | "__start" | "Reset_Handler" | "main")
}

fn normalize_archive_member(item: &crate::model::ArchiveContribution) -> String {
    match item.member_path.as_deref() {
        Some(member) => normalize_object_query(&format!("{}({member})", item.archive_path)),
        None => normalize_object_query(&item.archive_path),
    }
}

fn normalize_object_query(value: &str) -> String {
    if let Some((archive, member)) = value.split_once('(').and_then(|(archive, rest)| rest.strip_suffix(')').map(|member| (archive, member))) {
        return format!("{archive}:{member}");
    }
    value.replace('\\', "/")
}

fn object_node_id(path: &str, kind: ObjectSourceKind) -> String {
    format!("object:{}:{}", kind_label(kind), path)
}

fn kind_label(kind: ObjectSourceKind) -> &'static str {
    match kind {
        ObjectSourceKind::Object => "object",
        ObjectSourceKind::Internal => "internal",
    }
}

#[cfg(test)]
mod tests {
    use super::{build_linkage_graph, explain_object, explain_section, explain_symbol, explain_top_growth, Confidence};
    use crate::model::{
        AnalysisResult, ArchiveContribution, BinaryInfo, CrossReference, DebugArtifactInfo, DebugInfoSummary,
        DiffChangeKind, DiffEntry, DiffResult, DiffSummary, LinkerFamily, LinkerScriptInfo, MapFormat, MemorySummary,
        ObjectContribution, ObjectSourceKind, SectionCategory, SectionInfo, SectionPlacement, SymbolInfo, ToolchainInfo,
        ToolchainKind, ToolchainSelection, UnknownSourceBucket,
    };

    #[test]
    fn explains_archive_member_with_map_evidence() {
        let analysis = sample_analysis();
        let explain = explain_object(&analysis, "libapp.a(startup.o)").unwrap();
        assert_eq!(explain.confidence, Confidence::High);
        assert!(explain.summary.contains("linked to satisfy"));
        assert!(explain.evidence.iter().any(|item| item.source == "map.cref"));
    }

    #[test]
    fn explains_keep_section_with_high_confidence() {
        let analysis = sample_analysis();
        let explain = explain_section(&analysis, ".isr_vector").unwrap();
        assert_eq!(explain.confidence, Confidence::High);
        assert!(explain.evidence.iter().any(|item| item.detail.contains("KEEP")));
    }

    #[test]
    fn symbol_falls_back_to_candidate_object_when_direct_object_is_missing() {
        let mut analysis = sample_analysis();
        analysis.symbols[0].name = "worker_tick".to_string();
        let explain = explain_symbol(&analysis, "worker_tick").unwrap();
        assert_eq!(explain.confidence, Confidence::Low);
        assert!(explain.summary.contains("Candidate contributing object"));
    }

    #[test]
    fn builds_graph_and_top_growth_explanations() {
        let analysis = sample_analysis();
        let graph = build_linkage_graph(&analysis);
        assert!(graph.nodes.iter().any(|item| item.name == ".text"));
        assert!(graph.edges.iter().any(|item| item.detail.as_deref().unwrap_or("").contains("KEEP")));

        let diff = DiffResult {
            rom_delta: 8,
            ram_delta: 0,
            unknown_source_delta: 0,
            summary: DiffSummary::default(),
            section_diffs: Vec::new(),
            symbol_diffs: vec![DiffEntry {
                name: "main".to_string(),
                current: 8,
                previous: 0,
                delta: 8,
                change: DiffChangeKind::Added,
            }],
            object_diffs: vec![DiffEntry {
                name: "libapp.a:startup.o".to_string(),
                current: 16,
                previous: 0,
                delta: 16,
                change: DiffChangeKind::Added,
            }],
            archive_diffs: Vec::new(),
            source_file_diffs: Vec::new(),
            function_diffs: Vec::new(),
            line_diffs: Vec::new(),
        };
        let why = explain_top_growth(&analysis, &diff, 3);
        assert_eq!(why.top_symbols.len(), 1);
        assert_eq!(why.top_objects.len(), 1);
    }

    fn sample_analysis() -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "build/app.elf".to_string(),
                arch: "arm".to_string(),
                elf_class: "ELF32".to_string(),
                endian: "little".to_string(),
            },
            toolchain: ToolchainInfo {
                requested: ToolchainSelection::Auto,
                detected: Some(ToolchainKind::Gnu),
                resolved: ToolchainKind::Gnu,
                linker_family: LinkerFamily::Gnu,
                map_format: MapFormat::Gnu,
                parser_warnings_count: 0,
            },
            debug_info: DebugInfoSummary::default(),
            debug_artifact: DebugArtifactInfo::default(),
            sections: vec![
                SectionInfo {
                    name: ".text".to_string(),
                    addr: 0,
                    size: 0x40,
                    flags: vec!["ALLOC".to_string()],
                    category: SectionCategory::Rom,
                },
                SectionInfo {
                    name: ".isr_vector".to_string(),
                    addr: 0x40,
                    size: 0x10,
                    flags: vec!["ALLOC".to_string()],
                    category: SectionCategory::Rom,
                },
            ],
            symbols: vec![SymbolInfo {
                name: "main".to_string(),
                demangled_name: None,
                section_name: Some(".text".to_string()),
                object_path: None,
                addr: 0,
                size: 8,
            }],
            object_contributions: vec![ObjectContribution {
                object_path: "build/main.o".to_string(),
                source_kind: ObjectSourceKind::Object,
                section_name: Some(".text".to_string()),
                size: 0x20,
            }],
            archive_contributions: vec![ArchiveContribution {
                archive_path: "libapp.a".to_string(),
                member_path: Some("startup.o".to_string()),
                section_name: Some(".isr_vector".to_string()),
                size: 0x10,
            }],
            archive_pulls: vec![crate::model::ArchivePullDetail {
                archive_member: "libapp.a(startup.o)".to_string(),
                referenced_by: "build/main.o".to_string(),
                symbol: "startup_entry".to_string(),
            }],
            cross_references: vec![CrossReference {
                symbol: "startup_entry".to_string(),
                defined_in: "libapp.a(startup.o)".to_string(),
                referenced_by: vec!["build/main.o".to_string()],
            }],
            linker_script: Some(LinkerScriptInfo {
                path: "sample.ld".to_string(),
                regions: Vec::new(),
                placements: vec![
                    SectionPlacement {
                        section_name: ".text".to_string(),
                        region_name: "FLASH".to_string(),
                        load_region_name: None,
                        align: None,
                        keep: false,
                        has_at: false,
                    },
                    SectionPlacement {
                        section_name: ".isr_vector".to_string(),
                        region_name: "FLASH".to_string(),
                        load_region_name: None,
                        align: None,
                        keep: true,
                        has_at: false,
                    },
                ],
            }),
            memory: MemorySummary {
                rom_bytes: 0x50,
                ram_bytes: 0,
                section_totals: Vec::new(),
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
