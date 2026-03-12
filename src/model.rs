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
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObjectContribution {
    pub object_path: String,
    pub section_name: Option<String>,
    pub size: u64,
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
    pub sections: Vec<SectionInfo>,
    pub symbols: Vec<SymbolInfo>,
    pub object_contributions: Vec<ObjectContribution>,
    pub archive_contributions: Vec<ArchiveContribution>,
    pub linker_script: Option<LinkerScriptInfo>,
    pub memory: MemorySummary,
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
    pub summary: DiffSummary,
    pub section_diffs: Vec<DiffEntry>,
    pub symbol_diffs: Vec<DiffEntry>,
    pub object_diffs: Vec<DiffEntry>,
    pub archive_diffs: Vec<DiffEntry>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolchainKind {
    Gnu,
    Lld,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThresholdConfig {
    pub rom_percent: f64,
    pub ram_percent: f64,
    pub region_default_percent: f64,
    pub region_percent: BTreeMap<String, f64>,
    pub symbol_growth_bytes: u64,
    pub region_low_free_bytes: u64,
    pub section_growth_rate: f64,
    pub large_symbol_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DemangleMode {
    #[default]
    Auto,
    On,
    Off,
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
    pub warn_if_greater_than: Option<f64>,
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
            symbol_growth_bytes: 4 * 1024,
            region_low_free_bytes: 4 * 1024,
            section_growth_rate: 5.0,
            large_symbol_bytes: 4 * 1024,
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
