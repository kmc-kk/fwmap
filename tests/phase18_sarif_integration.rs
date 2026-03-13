use fwmap::model::{
    AddressRange, AnalysisResult, BinaryInfo, CompilationUnit, DebugArtifactInfo, DebugInfoSummary, FunctionAttribution,
    LineAttribution, LineRangeAttribution, LinkerFamily, MapFormat, MemorySummary, ObjectContribution,
    ObjectSourceKind, SectionCategory, SectionInfo, SourceFile, SourceLocation, SourceSpan, SymbolInfo, ToolchainInfo,
    ToolchainKind, ToolchainSelection, UnknownSourceBucket, WarningItem, WarningLevel, WarningSource,
};
use fwmap::sarif::{build_sarif_json, SarifOptions};

#[test]
fn github_minimal_fields_are_present_in_sarif_output() {
    let analysis = sample_analysis();
    let json = build_sarif_json(
        &analysis,
        &SarifOptions {
            base_uri: Some("file:///workspace/".to_string()),
            min_level: WarningLevel::Info,
            tool_name: "fwmap".to_string(),
            ..SarifOptions::default()
        },
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(value["version"], "2.1.0");
    assert_eq!(value["runs"][0]["tool"]["driver"]["name"], "fwmap");
    assert_eq!(value["runs"][0]["results"][0]["ruleId"], "LARGE_SYMBOL");
    assert_eq!(value["runs"][0]["results"][0]["level"], "warning");
    assert_eq!(
        value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "src/main.c"
    );
    assert!(value["runs"][0]["results"][0]["partialFingerprints"]["fwmap/v1"]
        .as_str()
        .unwrap()
        .len()
        >= 8);
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
        archive_pulls: Vec::new(),
        relocation_references: Vec::new(),
        cross_references: Vec::new(),
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
                line: 42,
                column: Some(3),
            },
            span: SourceSpan {
                path: "src/main.c".to_string(),
                line_start: 42,
                line_end: 42,
                column: Some(3),
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
            line_start: 42,
            line_end: 42,
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
                line_start: 42,
                line_end: 42,
                column: Some(3),
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
