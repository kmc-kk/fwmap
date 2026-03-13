use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::model::{AnalysisResult, WarningItem, WarningLevel, WarningSource};

#[derive(Debug, Clone)]
pub struct SarifOptions {
    pub base_uri: Option<String>,
    pub min_level: WarningLevel,
    pub include_pass: bool,
    pub tool_name: String,
}

impl Default for SarifOptions {
    fn default() -> Self {
        Self {
            base_uri: None,
            min_level: WarningLevel::Warn,
            include_pass: false,
            tool_name: env!("CARGO_PKG_NAME").to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLog {
    pub version: String,
    #[serde(rename = "$schema")]
    pub schema_uri: String,
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub tool: Tool,
    pub results: Vec<SarifResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_uri_base_ids: Option<BTreeMap<String, ArtifactLocation>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    pub driver: Driver,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Driver {
    pub name: String,
    pub version: String,
    pub information_uri: String,
    pub rules: Vec<ReportingDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportingDescriptor {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_description: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_description: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifResult {
    pub rule_id: String,
    pub level: String,
    pub kind: String,
    pub message: Message,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub locations: Vec<Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_fingerprints: Option<PartialFingerprints>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub physical_location: PhysicalLocation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicalLocation {
    pub artifact_location: ArtifactLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_column: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactLocation {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri_base_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartialFingerprints {
    #[serde(rename = "fwmap/v1")]
    pub fwmap_v1: String,
}

pub fn write_sarif_report(path: &Path, current: &AnalysisResult, options: &SarifOptions) -> Result<(), String> {
    let json = build_sarif_json(current, options)?;
    std::fs::write(path, json).map_err(|err| format!("failed to write SARIF report '{}': {err}", path.display()))
}

pub fn build_sarif_json(current: &AnalysisResult, options: &SarifOptions) -> Result<String, String> {
    let log = build_sarif_log(current, options);
    serde_json::to_string_pretty(&log).map_err(|err| format!("failed to serialize SARIF report: {err}"))
}

pub fn build_sarif_log(current: &AnalysisResult, options: &SarifOptions) -> SarifLog {
    let rules = collect_rules(&current.warnings);
    let results = current
        .warnings
        .iter()
        .filter(|item| item.level >= options.min_level)
        .map(|item| warning_to_result(current, item, options))
        .collect::<Vec<_>>();

    let original_uri_base_ids = options.base_uri.as_ref().map(|uri| {
        let mut ids = BTreeMap::new();
        ids.insert(
            "SRCROOT".to_string(),
            ArtifactLocation {
                uri: uri.clone(),
                uri_base_id: None,
            },
        );
        ids
    });

    SarifLog {
        version: "2.1.0".to_string(),
        schema_uri: "https://json.schemastore.org/sarif-2.1.0.json".to_string(),
        runs: vec![Run {
            tool: Tool {
                driver: Driver {
                    name: options.tool_name.clone(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    information_uri: option_env!("CARGO_PKG_REPOSITORY").unwrap_or("").to_string(),
                    rules,
                },
            },
            results,
            original_uri_base_ids,
        }],
    }
}

fn collect_rules(items: &[WarningItem]) -> Vec<ReportingDescriptor> {
    let mut seen = BTreeSet::new();
    let mut rules = Vec::new();
    for item in items {
        if !seen.insert(item.code.clone()) {
            continue;
        }
        rules.push(ReportingDescriptor {
            id: item.code.clone(),
            name: item.code.clone(),
            short_description: Some(Message {
                text: item.message.clone(),
            }),
            full_description: Some(Message {
                text: format!("{} warning from {}", item.code, item.source),
            }),
            help: Some(Message {
                text: format!(
                    "fwmap reported {} with severity {}. Review the related entity and report context for the underlying evidence.",
                    item.code, item.level
                ),
            }),
            help_uri: option_env!("CARGO_PKG_REPOSITORY")
                .map(|repo| format!("{repo}/blob/main/README.md#{}", item.code.to_ascii_lowercase())),
        });
    }
    rules.sort_by(|a, b| a.id.cmp(&b.id));
    rules
}

fn warning_to_result(current: &AnalysisResult, item: &WarningItem, options: &SarifOptions) -> SarifResult {
    let logical = logical_identity(current, item);
    let location = find_location(current, item, options);
    let fingerprint = stable_hash(&format!("{}|{}|{}", item.code, item.source, logical));

    let mut properties = BTreeMap::new();
    properties.insert("source".to_string(), serde_json::json!(item.source.to_string()));
    properties.insert("logicalIdentity".to_string(), serde_json::json!(logical));
    if let Some(related) = item.related.as_deref() {
        properties.insert("related".to_string(), serde_json::json!(related));
        if current.symbols.iter().any(|symbol| symbol.name == related) {
            properties.insert("symbol".to_string(), serde_json::json!(related));
        }
    }
    properties.insert("includePass".to_string(), serde_json::json!(options.include_pass));

    SarifResult {
        rule_id: item.code.clone(),
        level: sarif_level(item.level).to_string(),
        kind: "fail".to_string(),
        message: Message {
            text: item.message.clone(),
        },
        locations: location.into_iter().collect(),
        partial_fingerprints: Some(PartialFingerprints { fwmap_v1: fingerprint }),
        properties: Some(properties),
    }
}

fn logical_identity(current: &AnalysisResult, item: &WarningItem) -> String {
    if let Some(related) = item.related.as_deref() {
        if let Some(function) = current
            .function_attributions
            .iter()
            .find(|function| function.raw_name == related || function.demangled_name.as_deref() == Some(related))
        {
            if let Some(path) = function.path.as_deref() {
                return format!("function:{path}:{related}");
            }
        }
        if current.source_files.iter().any(|file| file.path == related) {
            return format!("file:{related}");
        }
        if current.sections.iter().any(|section| section.name == related) {
            return format!("section:{related}");
        }
        return format!("related:{related}");
    }
    format!("message:{}", item.message)
}

fn find_location(current: &AnalysisResult, item: &WarningItem, options: &SarifOptions) -> Option<Location> {
    if let Some(related) = item.related.as_deref() {
        if let Some(location) = line_location_for_path(current, related, options) {
            return Some(location);
        }
        if let Some(location) = function_location(current, related, options) {
            return Some(location);
        }
        if let Some(location) = symbol_location(current, related, options) {
            return Some(location);
        }
        if let Some(location) = section_location(current, related, options) {
            return Some(location);
        }
        if let Some(location) = file_only_location(related, options) {
            return Some(location);
        }
    }

    match item.source {
        WarningSource::Elf => file_only_location(&current.binary.path, options),
        WarningSource::Map | WarningSource::Analyze => None,
    }
}

fn line_location_for_path(current: &AnalysisResult, path: &str, options: &SarifOptions) -> Option<Location> {
    let line = current
        .line_attributions
        .iter()
        .filter(|item| item.location.path == path || item.span.path == path)
        .min_by_key(|item| (item.location.line, item.location.column.unwrap_or(0)))?;
    Some(make_location(
        &line.location.path,
        Some(line.location.line),
        Some(line.span.line_end.max(line.location.line)),
        line.location.column,
        options,
    ))
}

fn function_location(current: &AnalysisResult, related: &str, options: &SarifOptions) -> Option<Location> {
    let function = current
        .function_attributions
        .iter()
        .find(|item| item.raw_name == related || item.demangled_name.as_deref() == Some(related))?;
    let range = function.ranges.first()?;
    Some(make_location(
        &range.path,
        Some(range.line_start),
        Some(range.line_end),
        range.column,
        options,
    ))
}

fn symbol_location(current: &AnalysisResult, related: &str, options: &SarifOptions) -> Option<Location> {
    let symbol = current
        .symbols
        .iter()
        .find(|item| item.name == related || item.demangled_name.as_deref() == Some(related))?;
    let line = current
        .line_attributions
        .iter()
        .find(|item| symbol.addr >= item.range.start && symbol.addr < item.range.end)?;
    Some(make_location(
        &line.location.path,
        Some(line.location.line),
        Some(line.span.line_end.max(line.location.line)),
        line.location.column,
        options,
    ))
}

fn section_location(current: &AnalysisResult, related: &str, options: &SarifOptions) -> Option<Location> {
    let line = current
        .line_attributions
        .iter()
        .find(|item| item.range.section_name.as_deref() == Some(related))?;
    Some(make_location(
        &line.location.path,
        Some(line.location.line),
        Some(line.span.line_end.max(line.location.line)),
        line.location.column,
        options,
    ))
}

fn file_only_location(path: &str, options: &SarifOptions) -> Option<Location> {
    if path.is_empty() {
        return None;
    }
    Some(make_location(path, None, None, None, options))
}

fn make_location(
    path: &str,
    start_line: Option<u64>,
    end_line: Option<u64>,
    start_column: Option<u64>,
    options: &SarifOptions,
) -> Location {
    let artifact_location = artifact_location(path, options);
    Location {
        physical_location: PhysicalLocation {
            artifact_location,
            region: if start_line.is_some() || start_column.is_some() {
                Some(Region {
                    start_line,
                    end_line,
                    start_column,
                })
            } else {
                None
            },
        },
    }
}

fn artifact_location(path: &str, options: &SarifOptions) -> ArtifactLocation {
    let path = PathBuf::from(path);
    if let Some(base_uri) = options.base_uri.as_deref() {
        if let Ok(cwd) = std::env::current_dir() {
            if let Ok(relative) = path.strip_prefix(&cwd) {
                return ArtifactLocation {
                    uri: normalize_path(relative),
                    uri_base_id: Some("SRCROOT".to_string()),
                };
            }
        }
        if !path.is_absolute() {
            return ArtifactLocation {
                uri: normalize_path(&path),
                uri_base_id: Some("SRCROOT".to_string()),
            };
        }
        if base_uri.starts_with("file://") {
            return ArtifactLocation {
                uri: normalize_path(&path),
                uri_base_id: None,
            };
        }
    }
    ArtifactLocation {
        uri: normalize_path(&path),
        uri_base_id: None,
    }
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sarif_level(level: WarningLevel) -> &'static str {
    match level {
        WarningLevel::Info => "note",
        WarningLevel::Warn => "warning",
        WarningLevel::Error => "error",
    }
}

fn stable_hash(text: &str) -> String {
    let mut hasher = Fnv1a64::default();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[derive(Default)]
struct Fnv1a64(u64);

impl Hasher for Fnv1a64 {
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = if self.0 == 0 { 0xcbf29ce484222325 } else { self.0 };
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        self.0 = hash;
    }

    fn finish(&self) -> u64 {
        if self.0 == 0 { 0xcbf29ce484222325 } else { self.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::{build_sarif_log, SarifOptions};
    use crate::model::{
        AddressRange, AnalysisResult, BinaryInfo, CompilationUnit, DebugArtifactInfo, DebugInfoSummary,
        FunctionAttribution, LineAttribution, LineRangeAttribution, LinkerFamily, MapFormat, MemorySummary,
        ObjectContribution, ObjectSourceKind, SectionCategory, SectionInfo, SourceFile, SourceLocation, SourceSpan,
        SymbolInfo, ToolchainInfo, ToolchainKind, ToolchainSelection, UnknownSourceBucket, WarningItem, WarningLevel,
        WarningSource,
    };

    #[test]
    fn line_level_warning_maps_to_sarif_region() {
        let analysis = sample_analysis();
        let log = build_sarif_log(
            &analysis,
            &SarifOptions {
                min_level: WarningLevel::Info,
                ..SarifOptions::default()
            },
        );
        let result = log.runs[0].results.iter().find(|item| item.rule_id == "LARGE_SYMBOL").unwrap();
        assert_eq!(result.level, "warning");
        let location = &result.locations[0].physical_location;
        assert_eq!(location.artifact_location.uri, "src/main.c");
        assert_eq!(location.region.as_ref().unwrap().start_line, Some(10));
    }

    #[test]
    fn file_level_warning_maps_to_artifact_only() {
        let mut analysis = sample_analysis();
        analysis.warnings.push(WarningItem {
            level: WarningLevel::Info,
            code: "DEBUG_ARTIFACT_NOT_FOUND".to_string(),
            message: "No usable debug artifact was found".to_string(),
            source: WarningSource::Elf,
            related: Some("build/app.elf".to_string()),
        });
        let log = build_sarif_log(
            &analysis,
            &SarifOptions {
                min_level: WarningLevel::Info,
                ..SarifOptions::default()
            },
        );
        let result = log
            .runs[0]
            .results
            .iter()
            .find(|item| item.rule_id == "DEBUG_ARTIFACT_NOT_FOUND")
            .unwrap();
        assert!(result.locations[0].physical_location.region.is_none());
    }

    #[test]
    fn symbol_warning_keeps_symbol_property() {
        let analysis = sample_analysis();
        let log = build_sarif_log(
            &analysis,
            &SarifOptions {
                min_level: WarningLevel::Info,
                ..SarifOptions::default()
            },
        );
        let result = log.runs[0].results.iter().find(|item| item.rule_id == "LARGE_SYMBOL").unwrap();
        assert_eq!(result.properties.as_ref().unwrap().get("symbol").unwrap(), "main");
    }

    #[test]
    fn fingerprint_is_stable_for_same_logical_identity() {
        let analysis = sample_analysis();
        let log_a = build_sarif_log(
            &analysis,
            &SarifOptions {
                min_level: WarningLevel::Info,
                ..SarifOptions::default()
            },
        );
        let mut changed = sample_analysis();
        changed.line_attributions[0].location.line = 99;
        changed.line_attributions[0].span.line_start = 99;
        changed.line_attributions[0].span.line_end = 99;
        let log_b = build_sarif_log(
            &changed,
            &SarifOptions {
                min_level: WarningLevel::Info,
                ..SarifOptions::default()
            },
        );
        let a = log_a.runs[0].results[0].partial_fingerprints.as_ref().unwrap().fwmap_v1.clone();
        let b = log_b.runs[0].results[0].partial_fingerprints.as_ref().unwrap().fwmap_v1.clone();
        assert_eq!(a, b);
    }

    fn sample_analysis() -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "src/main.c".to_string(),
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
            sections: vec![SectionInfo {
                name: ".text".to_string(),
                addr: 0x1000,
                size: 16,
                flags: vec!["ALLOC".to_string()],
                category: SectionCategory::Rom,
            }],
            symbols: vec![SymbolInfo {
                name: "main".to_string(),
                demangled_name: None,
                section_name: Some(".text".to_string()),
                object_path: None,
                addr: 0x1000,
                size: 8,
            }],
            object_contributions: vec![ObjectContribution {
                object_path: "main.o".to_string(),
                source_kind: ObjectSourceKind::Object,
                section_name: Some(".text".to_string()),
                size: 16,
            }],
            archive_contributions: Vec::new(),
            linker_script: None,
            memory: MemorySummary {
                rom_bytes: 16,
                ram_bytes: 0,
                section_totals: Vec::new(),
                memory_regions: Vec::new(),
                region_summaries: Vec::new(),
            },
            compilation_units: vec![CompilationUnit {
                name: Some("main.c".to_string()),
                comp_dir: Some("src".to_string()),
                file_count: 1,
            }],
            source_files: vec![SourceFile {
                path: "src/main.c".to_string(),
                display_path: "src/main.c".to_string(),
                directory: "src".to_string(),
                size: 16,
                functions: 1,
                line_ranges: 1,
            }],
            line_attributions: vec![LineAttribution {
                location: SourceLocation {
                    path: "src/main.c".to_string(),
                    line: 10,
                    column: Some(1),
                },
                span: SourceSpan {
                    path: "src/main.c".to_string(),
                    line_start: 10,
                    line_end: 10,
                    column: Some(1),
                },
                range: AddressRange {
                    start: 0x1000,
                    end: 0x1008,
                    section_name: Some(".text".to_string()),
                },
                size: 8,
            }],
            line_hotspots: vec![LineRangeAttribution {
                path: "src/main.c".to_string(),
                line_start: 10,
                line_end: 10,
                section_name: Some(".text".to_string()),
                size: 8,
            }],
            function_attributions: vec![FunctionAttribution {
                raw_name: "main".to_string(),
                demangled_name: None,
                path: Some("src/main.c".to_string()),
                size: 8,
                ranges: vec![SourceSpan {
                    path: "src/main.c".to_string(),
                    line_start: 10,
                    line_end: 10,
                    column: Some(1),
                }],
            }],
            unknown_source: UnknownSourceBucket::default(),
            warnings: vec![WarningItem {
                level: WarningLevel::Warn,
                code: "LARGE_SYMBOL".to_string(),
                message: "Symbol main exceeded the threshold".to_string(),
                source: WarningSource::Analyze,
                related: Some("main".to_string()),
            }],
        }
    }
}
