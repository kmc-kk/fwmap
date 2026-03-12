use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryInfo {
    pub path: String,
    pub arch: String,
    pub elf_class: String,
    pub endian: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionInfo {
    pub name: String,
    pub addr: u64,
    pub size: u64,
    pub flags: Vec<String>,
    pub category: SectionCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolInfo {
    pub name: String,
    pub demangled_name: Option<String>,
    pub section_name: Option<String>,
    pub object_path: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectContribution {
    pub object_path: String,
    pub section_name: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveContribution {
    pub archive_path: String,
    pub member_path: Option<String>,
    pub section_name: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionTotal {
    pub section_name: String,
    pub size: u64,
    pub category: SectionCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRegion {
    pub name: String,
    pub origin: u64,
    pub length: u64,
    pub attributes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySummary {
    pub rom_bytes: u64,
    pub ram_bytes: u64,
    pub section_totals: Vec<SectionTotal>,
    pub memory_regions: Vec<MemoryRegion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarningItem {
    pub level: WarningLevel,
    pub code: String,
    pub message: String,
    pub source: WarningSource,
    pub related: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisResult {
    pub binary: BinaryInfo,
    pub sections: Vec<SectionInfo>,
    pub symbols: Vec<SymbolInfo>,
    pub object_contributions: Vec<ObjectContribution>,
    pub archive_contributions: Vec<ArchiveContribution>,
    pub memory: MemorySummary,
    pub warnings: Vec<WarningItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffEntry {
    pub name: String,
    pub current: u64,
    pub previous: u64,
    pub delta: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffResult {
    pub rom_delta: i64,
    pub ram_delta: i64,
    pub section_diffs: Vec<DiffEntry>,
    pub symbol_diffs: Vec<DiffEntry>,
    pub object_diffs: Vec<DiffEntry>,
    pub added_symbols: Vec<String>,
    pub removed_symbols: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarningLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningSource {
    Elf,
    Map,
    Analyze,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SectionCategory {
    Rom,
    Ram,
    Other,
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
