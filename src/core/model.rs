use std::fmt;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BinaryInfo {
    pub path: String,
    pub arch: String,
    pub elf_class: String,
    pub endian: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionInfo {
    pub name: String,
    pub addr: u64,
    pub size: u64,
    pub flags: Vec<String>,
    pub category: SectionCategory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolInfo {
    pub name: String,
    pub demangled_name: Option<String>,
    pub section_name: Option<String>,
    pub object_path: Option<String>,
    pub addr: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObjectContribution {
    pub object_path: String,
    pub source_kind: ObjectSourceKind,
    pub section_name: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectSourceKind {
    Object,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArchiveContribution {
    pub archive_path: String,
    pub member_path: Option<String>,
    pub section_name: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionTotal {
    pub section_name: String,
    pub size: u64,
    pub category: SectionCategory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemoryRegion {
    pub name: String,
    pub origin: u64,
    pub length: u64,
    pub attributes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionPlacement {
    pub section_name: String,
    pub region_name: String,
    pub load_region_name: Option<String>,
    pub align: Option<u64>,
    pub keep: bool,
    pub has_at: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkerScriptInfo {
    pub path: String,
    pub regions: Vec<MemoryRegion>,
    pub placements: Vec<SectionPlacement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegionSectionUsage {
    pub section_name: String,
    pub addr: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegionUsageSummary {
    pub region_name: String,
    pub origin: u64,
    pub length: u64,
    pub used: u64,
    pub free: u64,
    pub usage_ratio: f64,
    pub sections: Vec<RegionSectionUsage>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemorySummary {
    pub rom_bytes: u64,
    pub ram_bytes: u64,
    pub section_totals: Vec<SectionTotal>,
    pub memory_regions: Vec<MemoryRegion>,
    pub region_summaries: Vec<RegionUsageSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WarningItem {
    pub level: WarningLevel,
    pub code: String,
    pub message: String,
    pub source: WarningSource,
    pub related: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnalysisResult {
    pub binary: BinaryInfo,
    pub toolchain: ToolchainInfo,
    pub debug_info: DebugInfoSummary,
    pub sections: Vec<SectionInfo>,
    pub symbols: Vec<SymbolInfo>,
    pub object_contributions: Vec<ObjectContribution>,
    pub archive_contributions: Vec<ArchiveContribution>,
    pub linker_script: Option<LinkerScriptInfo>,
    pub memory: MemorySummary,
    pub compilation_units: Vec<CompilationUnit>,
    pub source_files: Vec<SourceFile>,
    pub line_attributions: Vec<LineAttribution>,
    pub line_hotspots: Vec<LineRangeAttribution>,
    pub function_attributions: Vec<FunctionAttribution>,
    pub unknown_source: UnknownSourceBucket,
    pub warnings: Vec<WarningItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffEntry {
    pub name: String,
    pub current: u64,
    pub previous: u64,
    pub delta: i64,
    pub change: DiffChangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffResult {
    pub rom_delta: i64,
    pub ram_delta: i64,
    pub unknown_source_delta: i64,
    pub summary: DiffSummary,
    pub section_diffs: Vec<DiffEntry>,
    pub symbol_diffs: Vec<DiffEntry>,
    pub object_diffs: Vec<DiffEntry>,
    pub archive_diffs: Vec<DiffEntry>,
    pub source_file_diffs: Vec<DiffEntry>,
    pub function_diffs: Vec<DiffEntry>,
    pub line_diffs: Vec<DiffEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct DiffSummary {
    pub section_added: usize,
    pub section_removed: usize,
    pub section_increased: usize,
    pub section_decreased: usize,
    pub symbol_added: usize,
    pub symbol_removed: usize,
    pub symbol_increased: usize,
    pub symbol_decreased: usize,
    pub object_added: usize,
    pub object_removed: usize,
    pub object_increased: usize,
    pub object_decreased: usize,
    pub source_file_added: usize,
    pub source_file_removed: usize,
    pub source_file_increased: usize,
    pub source_file_decreased: usize,
    pub function_added: usize,
    pub function_removed: usize,
    pub function_increased: usize,
    pub function_decreased: usize,
    pub line_added: usize,
    pub line_removed: usize,
    pub line_increased: usize,
    pub line_decreased: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum WarningLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolchainInfo {
    pub requested: ToolchainSelection,
    pub detected: Option<ToolchainKind>,
    pub resolved: ToolchainKind,
    pub linker_family: LinkerFamily,
    pub map_format: MapFormat,
    pub parser_warnings_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum WarningSource {
    Elf,
    Map,
    Analyze,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum SectionCategory {
    Rom,
    Ram,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DiffChangeKind {
    Added,
    Removed,
    Increased,
    Decreased,
    Unchanged,
    Moved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ToolchainSelection {
    #[default]
    Auto,
    Gnu,
    Lld,
    Iar,
    Armcc,
    Keil,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MapFormatSelection {
    #[default]
    Auto,
    Gnu,
    LldNative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolchainKind {
    Gnu,
    Lld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MapFormat {
    #[default]
    Unknown,
    Gnu,
    LldNative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LinkerFamily {
    #[default]
    Unknown,
    Gnu,
    Lld,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThresholdConfig {
    pub rom_percent: f64,
    pub ram_percent: f64,
    pub region_default_percent: f64,
    pub region_percent: BTreeMap<String, f64>,
    pub unknown_source_ratio: f64,
    pub symbol_growth_bytes: u64,
    pub region_low_free_bytes: u64,
    pub section_growth_rate: f64,
    pub large_symbol_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompilationUnit {
    pub name: Option<String>,
    pub comp_dir: Option<String>,
    pub file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceLocation {
    pub path: String,
    pub line: u64,
    pub column: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceSpan {
    pub path: String,
    pub line_start: u64,
    pub line_end: u64,
    pub column: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddressRange {
    pub start: u64,
    pub end: u64,
    pub section_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineAttribution {
    pub location: SourceLocation,
    pub span: SourceSpan,
    pub range: AddressRange,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FunctionAttribution {
    pub raw_name: String,
    pub demangled_name: Option<String>,
    pub path: Option<String>,
    pub size: u64,
    pub ranges: Vec<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineRangeAttribution {
    pub path: String,
    pub line_start: u64,
    pub line_end: u64,
    pub section_name: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct UnknownSourceBucket {
    pub size: u64,
    pub ranges: Vec<AddressRange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceFile {
    pub path: String,
    pub display_path: String,
    pub directory: String,
    pub size: u64,
    pub functions: usize,
    pub line_ranges: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DebugInfoSummary {
    pub dwarf_mode: DwarfMode,
    pub source_lines: SourceLinesMode,
    pub dwarf_used: bool,
    pub cache_hit: bool,
    pub split_dwarf_detected: bool,
    pub split_dwarf_kind: Option<String>,
    pub unknown_source_ratio: f64,
    pub compilation_units: usize,
    pub line_zero_ranges: usize,
    pub generated_ranges: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DemangleMode {
    #[default]
    Auto,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DwarfMode {
    #[default]
    Auto,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SourceLinesMode {
    #[default]
    Off,
    Files,
    Functions,
    Lines,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiFormat {
    Text,
    Markdown,
    Json,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleConfigFile {
    #[serde(default = "default_rule_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub thresholds: RuleThresholdOverrides,
    #[serde(default)]
    pub rules: Vec<CustomRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RuleThresholdOverrides {
    pub rom_usage_warn: Option<f64>,
    pub ram_usage_warn: Option<f64>,
    pub unknown_source_warn: Option<f64>,
    pub symbol_growth_warn_bytes: Option<u64>,
    pub large_symbol_warn_bytes: Option<u64>,
    pub section_growth_warn_percent: Option<f64>,
    pub region_low_free_warn_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomRule {
    pub id: String,
    pub kind: RuleKind,
    pub severity: RuleSeverityConfig,
    pub message: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub region: Option<String>,
    pub section: Option<String>,
    pub symbol: Option<String>,
    pub object: Option<String>,
    pub pattern: Option<String>,
    pub warn_if_greater_than: Option<f64>,
    pub threshold_bytes: Option<i64>,
    pub warn_if_delta_bytes_gt: Option<i64>,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub denylist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    RegionUsage,
    SectionDelta,
    SymbolDelta,
    SymbolMatch,
    ObjectMatch,
    SourcePathGrowth,
    FunctionGrowth,
    UnknownSourceRatio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleSeverityConfig {
    Info,
    Warn,
    Error,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            rom_percent: 85.0,
            ram_percent: 85.0,
            region_default_percent: 85.0,
            region_percent: BTreeMap::new(),
            unknown_source_ratio: 0.15,
            symbol_growth_bytes: 4 * 1024,
            region_low_free_bytes: 4 * 1024,
            section_growth_rate: 5.0,
            large_symbol_bytes: 4 * 1024,
        }
    }
}

impl Default for DebugInfoSummary {
    fn default() -> Self {
        Self {
            dwarf_mode: DwarfMode::Auto,
            source_lines: SourceLinesMode::Off,
            dwarf_used: false,
            cache_hit: false,
            split_dwarf_detected: false,
            split_dwarf_kind: None,
            unknown_source_ratio: 0.0,
            compilation_units: 0,
            line_zero_ranges: 0,
            generated_ranges: 0,
        }
    }
}

fn default_rule_schema_version() -> u32 {
    1
}

fn default_enabled() -> bool {
    true
}

impl fmt::Display for WarningLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            WarningLevel::Info => "info",
            WarningLevel::Warn => "warn",
            WarningLevel::Error => "error",
        };
        write!(f, "{text}")
    }
}

impl fmt::Display for SectionCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SectionCategory::Rom => "ROM",
            SectionCategory::Ram => "RAM",
            SectionCategory::Other => "Other",
        };
        write!(f, "{text}")
    }
}

impl fmt::Display for WarningSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            WarningSource::Elf => "elf",
            WarningSource::Map => "map",
            WarningSource::Analyze => "analyze",
        };
        write!(f, "{text}")
    }
}

impl fmt::Display for DiffChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            DiffChangeKind::Added => "Added",
            DiffChangeKind::Removed => "Removed",
            DiffChangeKind::Increased => "Increased",
            DiffChangeKind::Decreased => "Decreased",
            DiffChangeKind::Unchanged => "Unchanged",
            DiffChangeKind::Moved => "Moved",
        };
        write!(f, "{text}")
    }
}

impl fmt::Display for ToolchainSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            ToolchainSelection::Auto => "auto",
            ToolchainSelection::Gnu => "gnu",
            ToolchainSelection::Lld => "lld",
            ToolchainSelection::Iar => "iar",
            ToolchainSelection::Armcc => "armcc",
            ToolchainSelection::Keil => "keil",
        };
        write!(f, "{text}")
    }
}

impl fmt::Display for ToolchainKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            ToolchainKind::Gnu => "gnu",
            ToolchainKind::Lld => "lld",
        };
        write!(f, "{text}")
    }
}

impl fmt::Display for MapFormatSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            MapFormatSelection::Auto => "auto",
            MapFormatSelection::Gnu => "gnu",
            MapFormatSelection::LldNative => "lld-native",
        };
        write!(f, "{text}")
    }
}

impl fmt::Display for MapFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            MapFormat::Unknown => "unknown",
            MapFormat::Gnu => "gnu",
            MapFormat::LldNative => "lld-native",
        };
        write!(f, "{text}")
    }
}

impl fmt::Display for LinkerFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            LinkerFamily::Unknown => "unknown",
            LinkerFamily::Gnu => "gnu",
            LinkerFamily::Lld => "lld",
        };
        write!(f, "{text}")
    }
}

impl fmt::Display for DwarfMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            DwarfMode::Auto => "auto",
            DwarfMode::On => "on",
            DwarfMode::Off => "off",
        };
        write!(f, "{text}")
    }
}

impl fmt::Display for SourceLinesMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            SourceLinesMode::Off => "off",
            SourceLinesMode::Files => "files",
            SourceLinesMode::Functions => "functions",
            SourceLinesMode::Lines => "lines",
            SourceLinesMode::All => "all",
        };
        write!(f, "{text}")
    }
}
