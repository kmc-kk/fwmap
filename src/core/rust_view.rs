use std::collections::BTreeMap;

use crate::model::{
    AnalysisResult, RustAggregate, RustContext, RustFamilyKind, RustFamilySummary, RustSymbolSummary, RustView,
    SymbolInfo, SymbolLanguage,
};

pub fn build_rust_view(result: &AnalysisResult) -> Option<RustView> {
    let classified = result
        .symbols
        .iter()
        .filter_map(|symbol| classify_symbol(symbol, result.rust_context.as_ref(), result))
        .collect::<Vec<_>>();
    if classified.is_empty() {
        return None;
    }

    let packages = aggregate(&classified, |item| item.package.clone());
    let targets = aggregate(&classified, |item| item.target.clone());
    let crates = aggregate(&classified, |item| item.crate_name.clone());
    let dependency_crates = aggregate(&classified, |item| item.dependency_crate.clone());
    let source_files = aggregate(&classified, |item| item.source_path.clone());
    let grouped_families = aggregate_families(&classified);
    let total_rust_size = classified.iter().map(|item| item.size).sum();

    Some(RustView {
        workspace: result.rust_context.as_ref().and_then(|item| item.workspace_root.clone()),
        packages,
        targets,
        crates,
        dependency_crates,
        source_files,
        grouped_families,
        symbols: sorted_symbols(classified),
        total_rust_size,
    })
}

pub fn aggregate_group_sizes(view: &RustView, group_by: RustGroupBy) -> Vec<(String, u64)> {
    let mut totals = BTreeMap::<String, u64>::new();
    for symbol in &view.symbols {
        if let Some(key) = group_key(symbol, group_by) {
            *totals.entry(key).or_default() += symbol.size;
        }
    }
    totals.into_iter().collect()
}

pub fn top_group_symbols(view: &RustView, group_by: RustGroupBy, name: &str, limit: usize) -> Vec<String> {
    let mut members = view
        .symbols
        .iter()
        .filter(|item| group_key(item, group_by).as_deref() == Some(name))
        .map(|item| (item.display_name.clone(), item.size))
        .collect::<Vec<_>>();
    members.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    members.into_iter().take(limit).map(|item| item.0).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustGroupBy {
    Package,
    Target,
    Crate,
    Dependency,
    Family,
    Symbol,
    SourceFile,
}

fn group_key(symbol: &RustSymbolSummary, group_by: RustGroupBy) -> Option<String> {
    match group_by {
        RustGroupBy::Package => symbol.package.clone(),
        RustGroupBy::Target => symbol.target.clone(),
        RustGroupBy::Crate => symbol.crate_name.clone(),
        RustGroupBy::Dependency => symbol.dependency_crate.clone(),
        RustGroupBy::Family => Some(symbol.family_key.clone()),
        RustGroupBy::Symbol => Some(symbol.display_name.clone()),
        RustGroupBy::SourceFile => symbol.source_path.clone(),
    }
}

fn classify_symbol(symbol: &SymbolInfo, context: Option<&RustContext>, result: &AnalysisResult) -> Option<RustSymbolSummary> {
    let demangled = symbol.demangled_name.clone();
    let display_name = demangled.clone().unwrap_or_else(|| symbol.name.clone());
    if !looks_like_rust_symbol(symbol, demangled.as_deref()) {
        return None;
    }

    let source_path = result
        .function_attributions
        .iter()
        .find(|item| item.raw_name == symbol.name)
        .and_then(|item| item.path.clone())
        .or_else(|| {
            result
                .line_attributions
                .iter()
                .find(|item| item.range.start <= symbol.addr && item.range.end >= symbol.addr)
                .map(|item| item.location.path.clone())
        });
    let crate_name = derive_crate_name(&display_name, context);
    let package_name = context
        .and_then(|item| item.package_name.clone())
        .or_else(|| crate_name.clone());
    let dependency_crate = crate_name
        .clone()
        .filter(|crate_name| Some(crate_name.as_str()) != context.and_then(|item| item.package_name.as_deref()));
    let (family_kind, family_key, family_display) = classify_family(&display_name);

    Some(RustSymbolSummary {
        raw_name: symbol.name.clone(),
        demangled_name: demangled,
        display_name: family_display,
        language: SymbolLanguage::Rust,
        package: package_name,
        target: context.and_then(|item| item.target_name.clone()),
        crate_name,
        dependency_crate,
        source_path,
        family_kind,
        family_key,
        size: symbol.size,
    })
}

fn sorted_symbols(mut symbols: Vec<RustSymbolSummary>) -> Vec<RustSymbolSummary> {
    symbols.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.display_name.cmp(&b.display_name)));
    symbols
}

fn aggregate<F>(symbols: &[RustSymbolSummary], key_fn: F) -> Vec<RustAggregate>
where
    F: Fn(&RustSymbolSummary) -> Option<String>,
{
    let mut totals = BTreeMap::<String, (u64, usize)>::new();
    for item in symbols {
        if let Some(key) = key_fn(item) {
            let entry = totals.entry(key).or_default();
            entry.0 += item.size;
            entry.1 += 1;
        }
    }
    let mut rows = totals
        .into_iter()
        .map(|(name, (size, symbol_count))| RustAggregate {
            name,
            size,
            symbol_count,
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    rows.truncate(20);
    rows
}

fn aggregate_families(symbols: &[RustSymbolSummary]) -> Vec<RustFamilySummary> {
    let mut totals = BTreeMap::<String, (RustFamilyKind, String, u64, usize)>::new();
    for item in symbols {
        let entry = totals
            .entry(item.family_key.clone())
            .or_insert((item.family_kind.clone(), item.display_name.clone(), 0, 0));
        entry.2 += item.size;
        entry.3 += 1;
    }
    let mut rows = totals
        .into_iter()
        .map(|(key, (kind, display_name, size, symbol_count))| RustFamilySummary {
            kind,
            key,
            display_name,
            size,
            symbol_count,
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.display_name.cmp(&b.display_name)));
    rows.truncate(20);
    rows
}

fn looks_like_rust_symbol(symbol: &SymbolInfo, demangled: Option<&str>) -> bool {
    demangled
        .map(|value| value.contains("::"))
        .unwrap_or(false)
        || symbol.name.starts_with("_R")
        || symbol.name.starts_with("_ZN")
}

fn derive_crate_name(display_name: &str, context: Option<&RustContext>) -> Option<String> {
    if let Some(stripped) = display_name.strip_prefix('<') {
        let normalized = stripped.split(" as ").next().unwrap_or(stripped);
        if let Some(component) = normalized.split("::").next() {
            let cleaned = component.trim_matches('<').trim_matches('>');
            if !cleaned.is_empty() {
                return Some(cleaned.to_string());
            }
        }
    }
    if let Some(component) = display_name.split("::").next() {
        let cleaned = component.trim_matches('<').trim_matches('>');
        if !cleaned.is_empty() && cleaned != "_" {
            return Some(cleaned.to_string());
        }
    }
    context.and_then(|item| item.package_name.clone())
}

fn classify_family(display_name: &str) -> (RustFamilyKind, String, String) {
    let no_hash = strip_hash_suffix(display_name);
    if let Some(key) = normalize_async_family(&no_hash) {
        return (RustFamilyKind::Async, key.clone(), key);
    }
    if let Some(key) = normalize_closure_family(&no_hash) {
        return (RustFamilyKind::Closure, key.clone(), key);
    }
    if let Some(key) = normalize_trait_family(&no_hash) {
        return (RustFamilyKind::Trait, key.clone(), key);
    }
    if let Some(key) = normalize_generic_family(&no_hash) {
        return (RustFamilyKind::Generic, key.clone(), key);
    }
    let key = normalize_function_family(&no_hash);
    (RustFamilyKind::Function, key.clone(), key)
}

fn strip_hash_suffix(value: &str) -> String {
    value
        .rsplit_once("::h")
        .filter(|(_, hash)| hash.len() == 16 && hash.chars().all(|ch| ch.is_ascii_hexdigit()))
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_else(|| value.to_string())
}

fn normalize_generic_family(value: &str) -> Option<String> {
    value.contains('<').then(|| normalize_angle_brackets(value))
}

fn normalize_closure_family(value: &str) -> Option<String> {
    if !value.contains("{{closure}}") {
        return None;
    }
    Some(value.replace("::{{closure}}", "::{{closure}}"))
}

fn normalize_async_family(value: &str) -> Option<String> {
    if !(value.contains("{{async}}")
        || value.contains("::poll")
        || value.contains(" from ")
        || value.contains("Future")
        || value.contains("GenFuture"))
    {
        return None;
    }
    Some(normalize_angle_brackets(value))
}

fn normalize_trait_family(value: &str) -> Option<String> {
    (value.starts_with('<') && value.contains(" as ")).then(|| normalize_angle_brackets(value))
}

fn normalize_function_family(value: &str) -> String {
    normalize_angle_brackets(value)
}

fn normalize_angle_brackets(value: &str) -> String {
    let mut depth = 0usize;
    let mut output = String::new();
    let mut inserted = false;
    for ch in value.chars() {
        match ch {
            '<' => {
                depth += 1;
                if depth == 1 {
                    output.push('<');
                    output.push_str("...");
                    inserted = true;
                }
            }
            '>' => {
                if depth == 1 {
                    output.push('>');
                }
                depth = depth.saturating_sub(1);
            }
            _ if depth == 0 => output.push(ch),
            _ => {}
        }
    }
    if inserted { output } else { value.to_string() }
}

#[cfg(test)]
mod tests {
    use super::{build_rust_view, classify_family, derive_crate_name, RustGroupBy};
    use crate::demangle::demangle_symbol;
    use crate::model::{
        AnalysisResult, BinaryInfo, DebugArtifactInfo, DebugInfoSummary, DemangleMode, LinkerFamily, MapFormat,
        MemorySummary, RustContext, SectionInfo, SectionTotal, SymbolInfo, ToolchainInfo, ToolchainKind,
        ToolchainSelection, UnknownSourceBucket,
    };

    #[test]
    fn demangles_rust_v0_symbol() {
        let value = demangle_symbol("_RNvCs1234_4test4main", DemangleMode::On);
        assert!(value.is_some());
    }

    #[test]
    fn normalizes_generic_closure_async_families() {
        let (kind_generic, key_generic, _) = classify_family("app::foo::<alloc::vec::Vec<u8>>");
        assert_eq!(kind_generic, crate::model::RustFamilyKind::Generic);
        assert!(key_generic.contains("<...>"));

        let (kind_closure, _, _) = classify_family("app::main::{{closure}}");
        assert_eq!(kind_closure, crate::model::RustFamilyKind::Closure);

        let (kind_async, _, _) = classify_family("core::future::from_generator::GenFuture<app::run::{{closure}}>");
        assert_eq!(kind_async, crate::model::RustFamilyKind::Async);
    }

    #[test]
    fn derives_crate_name_from_demangled_path() {
        assert_eq!(
            derive_crate_name("serde_json::ser::to_string", None).as_deref(),
            Some("serde_json")
        );
    }

    #[test]
    fn builds_rust_view_from_symbols() {
        let mut analysis = stub_analysis();
        analysis.rust_context = Some(RustContext {
            workspace_root: Some("/workspace/fwmap".to_string()),
            manifest_path: Some("/workspace/fwmap/Cargo.toml".to_string()),
            package_name: Some("fwmap".to_string()),
            package_id: None,
            target_name: Some("fwmap".to_string()),
            target_kind: vec!["bin".to_string()],
            crate_types: vec!["bin".to_string()],
            edition: Some("2024".to_string()),
            target_triple: Some("x86_64-unknown-linux-gnu".to_string()),
            profile: Some("release".to_string()),
            artifact_path: None,
            metadata_source: "test".to_string(),
            workspace_members: vec!["fwmap".to_string()],
        });
        analysis.symbols = vec![
            SymbolInfo {
                name: "_RNvNtCs3fwmap3app4main".to_string(),
                demangled_name: Some("fwmap::app::main".to_string()),
                section_name: None,
                object_path: Some("target/release/deps/fwmap.o".to_string()),
                addr: 0,
                size: 100,
            },
            SymbolInfo {
                name: "_RNvNtCs5serde4ser9to_string".to_string(),
                demangled_name: Some("serde::ser::to_string".to_string()),
                section_name: None,
                object_path: Some("target/release/deps/libserde.rlib".to_string()),
                addr: 10,
                size: 40,
            },
        ];
        let view = build_rust_view(&analysis).unwrap();
        assert_eq!(view.total_rust_size, 140);
        assert_eq!(view.packages[0].name, "fwmap");
        assert_eq!(view.targets[0].name, "fwmap");
        assert_eq!(view.dependency_crates[0].name, "serde");
        assert_eq!(super::aggregate_group_sizes(&view, RustGroupBy::Crate).len(), 2);
    }

    #[test]
    fn degrades_gracefully_without_rust_symbols() {
        let analysis = stub_analysis();
        assert!(build_rust_view(&analysis).is_none());
    }

    fn stub_analysis() -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "a.elf".to_string(),
                arch: "x86_64".to_string(),
                elf_class: "ELF64".to_string(),
                endian: "little-endian".to_string(),
            },
            git: None,
            rust_context: None,
            rust_view: None,
            toolchain: ToolchainInfo {
                requested: ToolchainSelection::Auto,
                detected: None,
                resolved: ToolchainKind::Gnu,
                linker_family: LinkerFamily::Gnu,
                map_format: MapFormat::Unknown,
                parser_warnings_count: 0,
            },
            debug_info: DebugInfoSummary::default(),
            debug_artifact: DebugArtifactInfo::default(),
            policy: None,
            sections: vec![SectionInfo {
                name: ".text".to_string(),
                addr: 0,
                size: 0,
                flags: vec![],
                category: crate::model::SectionCategory::Rom,
            }],
            symbols: Vec::new(),
            object_contributions: Vec::new(),
            archive_contributions: Vec::new(),
            archive_pulls: Vec::new(),
            whole_archive_candidates: Vec::new(),
            relocation_references: Vec::new(),
            cross_references: Vec::new(),
            cpp_view: crate::model::CppView::default(),
            linker_script: None,
            memory: MemorySummary {
                rom_bytes: 0,
                ram_bytes: 0,
                section_totals: vec![SectionTotal {
                    section_name: ".text".to_string(),
                    size: 0,
                    category: crate::model::SectionCategory::Rom,
                }],
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
