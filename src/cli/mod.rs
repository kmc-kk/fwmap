use std::path::{Path, PathBuf};

use crate::analyze::{analyze_paths, evaluate_warnings, AnalyzeOptions};
use crate::diff::{diff_results, top_increases};
use crate::demangle::display_name;
use crate::history::{
    list_builds, print_build_detail, print_build_list, print_trend, record_build, show_build, trend_metric,
    HistoryRecordInput,
};
use crate::model::{
    CiFormat, DemangleMode, DwarfMode, SourceLinesMode, ThresholdConfig, ToolchainSelection, WarningLevel,
};
use crate::rule_config::{apply_threshold_overrides, load_rule_config};
use crate::render::{print_ci_summary, print_cli_summary, write_ci_summary, write_html_report, write_json_report};

const DEFAULT_OUT: &str = "fwmap_report.html";
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(args: impl IntoIterator<Item = String>) -> Result<i32, String> {
    let parsed = parse_args(args.into_iter().collect())?;
    match parsed {
        Command::Help => {
            print_help();
            Ok(0)
        }
        Command::Version => {
            println!("fwmap {VERSION}");
            Ok(0)
        }
        Command::HistoryRecord {
            db,
            elf,
            map,
            lds,
            thresholds,
            rules,
            demangle,
            toolchain,
            dwarf_mode,
            source_lines,
            source_root,
            path_remaps,
            fail_on_missing_dwarf,
            metadata,
        } => {
            let mut options = AnalyzeOptions {
                thresholds,
                demangle,
                custom_rules: Vec::new(),
                toolchain,
                dwarf_mode,
                source_lines,
                source_root,
                path_remaps,
                fail_on_missing_dwarf,
            };
            if let Some(rule_path) = rules.as_deref() {
                let config = load_rule_config(rule_path)?;
                apply_threshold_overrides(&mut options.thresholds, &config.thresholds);
                options.custom_rules = config.rules;
            }
            let analysis = analyze_paths(&elf, map.as_deref(), lds.as_deref(), &options)?;
            let id = record_build(&db, HistoryRecordInput { analysis, metadata })?;
            println!("Recorded build #{id} into {}", db.display());
            Ok(0)
        }
        Command::HistoryList { db } => {
            let items = list_builds(&db)?;
            print_build_list(&items);
            Ok(0)
        }
        Command::HistoryShow { db, build } => {
            match show_build(&db, build)? {
                Some(detail) => {
                    print_build_detail(&detail);
                    Ok(0)
                }
                None => Err(format!("build id {build} was not found in {}", db.display())),
            }
        }
        Command::HistoryTrend { db, metric, last } => {
            let points = trend_metric(&db, &metric, last)?;
            print_trend(&points);
            Ok(0)
        }
        Command::Analyze {
            elf,
            map,
            lds,
            prev_elf,
            prev_map,
            out,
            report_json,
            ci_summary,
            ci_format,
            ci_out,
            ci_source_summary,
            fail_on_warning,
            max_source_diff_items,
            min_line_diff_bytes,
            hide_unknown_source,
            thresholds,
            rules,
            demangle,
            toolchain,
            dwarf_mode,
            source_lines,
            source_root,
            path_remaps,
            fail_on_missing_dwarf,
            verbose,
        } => {
            let mut options = AnalyzeOptions {
                thresholds,
                demangle,
                custom_rules: Vec::new(),
                toolchain,
                dwarf_mode,
                source_lines,
                source_root,
                path_remaps,
                fail_on_missing_dwarf,
            };
            if let Some(rule_path) = rules.as_deref() {
                let config = load_rule_config(rule_path)?;
                apply_threshold_overrides(&mut options.thresholds, &config.thresholds);
                options.custom_rules = config.rules;
            }

            let mut current = analyze_paths(&elf, map.as_deref(), lds.as_deref(), &options)?;
            let diff = if let Some(prev_elf) = prev_elf.as_deref() {
                let previous = analyze_paths(prev_elf, prev_map.as_deref(), lds.as_deref(), &options)?;
                let diff = diff_results(&current, &previous);
                let mut warnings = current
                    .warnings
                    .iter()
                    .filter(|warning| warning.source != crate::model::WarningSource::Analyze)
                    .cloned()
                    .collect::<Vec<_>>();
                warnings.extend(evaluate_warnings(&current, Some(&diff), &options.thresholds, &options.custom_rules));
                current.warnings = warnings;
                Some(diff)
            } else {
                None
            };
            let ci_format = ci_format.or(if ci_summary { Some(CiFormat::Text) } else { None });
            if let Some(format) = ci_format {
                let source_options = crate::render::SourceRenderOptions {
                    enabled: ci_source_summary,
                    max_diff_items: max_source_diff_items,
                    min_line_diff_bytes,
                    hide_unknown_source,
                };
                print_ci_summary(&current, diff.as_ref(), format, source_options)?;
                if let Some(path) = ci_out.as_deref() {
                    write_ci_summary(path, &current, diff.as_ref(), format, source_options)?;
                }
            } else {
                print_cli_summary(&current, diff.as_ref(), verbose);
                if let Some(diff) = diff.as_ref() {
                    if let Some(symbol) = top_increases(&diff.symbol_diffs, 1).first() {
                        let display = current
                            .symbols
                            .iter()
                            .find(|item| item.name == symbol.name)
                            .map(display_name)
                            .unwrap_or(&symbol.name);
                        println!("Top growth symbol: {} ({:+})", display, symbol.delta);
                    }
                    if let Some(object) = top_increases(&diff.object_diffs, 1).first() {
                        println!("Top growth object: {} ({:+})", object.name, object.delta);
                    }
                }
            }
            let source_options = crate::render::SourceRenderOptions {
                enabled: ci_source_summary,
                max_diff_items: max_source_diff_items,
                min_line_diff_bytes,
                hide_unknown_source,
            };
            write_html_report(&out, &current, diff.as_ref(), source_options)?;
            if let Some(path) = report_json.as_deref() {
                write_json_report(path, &current, diff.as_ref(), &options.thresholds, source_options)?;
                if !ci_summary {
                    println!("JSON: {}", path.display());
                }
            }
            if !ci_summary {
                println!("Report: {}", out.display());
            }
            if current.warnings.iter().any(|warning| warning.level == WarningLevel::Error) {
                return Ok(2);
            }
            if fail_on_warning && !current.warnings.is_empty() {
                return Ok(1);
            }
            Ok(0)
        }
    }
}

#[derive(Debug)]
enum Command {
    Help,
    Version,
    HistoryRecord {
        db: PathBuf,
        elf: PathBuf,
        map: Option<PathBuf>,
        lds: Option<PathBuf>,
        thresholds: ThresholdConfig,
        rules: Option<PathBuf>,
        demangle: DemangleMode,
        toolchain: ToolchainSelection,
        dwarf_mode: DwarfMode,
        source_lines: SourceLinesMode,
        source_root: Option<PathBuf>,
        path_remaps: Vec<(String, String)>,
        fail_on_missing_dwarf: bool,
        metadata: std::collections::BTreeMap<String, String>,
    },
    HistoryList {
        db: PathBuf,
    },
    HistoryShow {
        db: PathBuf,
        build: i64,
    },
    HistoryTrend {
        db: PathBuf,
        metric: String,
        last: usize,
    },
    Analyze {
        elf: PathBuf,
        map: Option<PathBuf>,
        lds: Option<PathBuf>,
        prev_elf: Option<PathBuf>,
        prev_map: Option<PathBuf>,
        out: PathBuf,
        report_json: Option<PathBuf>,
        ci_summary: bool,
        ci_format: Option<CiFormat>,
        ci_out: Option<PathBuf>,
        ci_source_summary: bool,
        fail_on_warning: bool,
        max_source_diff_items: usize,
        min_line_diff_bytes: u64,
        hide_unknown_source: bool,
        thresholds: ThresholdConfig,
        rules: Option<PathBuf>,
        demangle: DemangleMode,
        toolchain: ToolchainSelection,
        dwarf_mode: DwarfMode,
        source_lines: SourceLinesMode,
        source_root: Option<PathBuf>,
        path_remaps: Vec<(String, String)>,
        fail_on_missing_dwarf: bool,
        verbose: bool,
    },
}

fn parse_args(args: Vec<String>) -> Result<Command, String> {
    if args.len() <= 1 || matches!(args.get(1).map(String::as_str), Some("--help" | "-h")) {
        return Ok(Command::Help);
    }
    if matches!(args.get(1).map(String::as_str), Some("--version" | "-V")) {
        return Ok(Command::Version);
    }
    if args[1] == "history" {
        return parse_history_args(args);
    }
    if args[1] != "analyze" {
        return Err(format!("unknown command '{}'\n\n{}", args[1], help_text()));
    }

    let mut elf = None;
    let mut map = None;
    let mut lds = None;
    let mut prev_elf = None;
    let mut prev_map = None;
    let mut out = PathBuf::from(DEFAULT_OUT);
    let mut report_json = None;
    let mut ci_summary = false;
    let mut ci_format = None;
    let mut ci_out = None;
    let mut ci_source_summary = false;
    let mut fail_on_warning = false;
    let mut max_source_diff_items = 10usize;
    let mut min_line_diff_bytes = 1u64;
    let mut hide_unknown_source = false;
    let mut thresholds = ThresholdConfig::default();
    let mut rules = None;
    let mut demangle = DemangleMode::Auto;
    let mut toolchain = ToolchainSelection::Auto;
    let mut dwarf_mode = DwarfMode::Auto;
    let mut source_lines = SourceLinesMode::Off;
    let mut source_root = None;
    let mut path_remaps = Vec::new();
    let mut fail_on_missing_dwarf = false;
    let mut verbose = false;
    let mut index = 2usize;
    while index < args.len() {
        let key = &args[index];
        match key.as_str() {
            "--verbose" => {
                verbose = true;
                index += 1;
                continue;
            }
            "--ci-summary" => {
                ci_summary = true;
                index += 1;
                continue;
            }
            "--ci-source-summary" => {
                ci_source_summary = true;
                index += 1;
                continue;
            }
            "--ci-format" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --ci-format".to_string())?;
                ci_format = Some(parse_ci_format(value)?);
                index += 2;
                continue;
            }
            "--fail-on-warning" => {
                fail_on_warning = true;
                index += 1;
                continue;
            }
            "--hide-unknown-source" => {
                hide_unknown_source = true;
                index += 1;
                continue;
            }
            "--fail-on-missing-dwarf" => {
                fail_on_missing_dwarf = true;
                index += 1;
                continue;
            }
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
            "--threshold-rom" | "--threshold-ram" | "--threshold-symbol-growth" | "--threshold-region" | "--max-source-diff-items" | "--min-line-diff-bytes" => {
                let value = args.get(index + 1).ok_or_else(|| format!("missing value for {key}"))?;
                match key.as_str() {
                    "--threshold-rom" => thresholds.rom_percent = parse_percent(value, key)?,
                    "--threshold-ram" => thresholds.ram_percent = parse_percent(value, key)?,
                    "--threshold-symbol-growth" => thresholds.symbol_growth_bytes = parse_u64(value, key)?,
                    "--threshold-region" => {
                        let (name, percent) = parse_region_threshold(value)?;
                        thresholds.region_percent.insert(name, percent);
                    }
                    "--max-source-diff-items" => max_source_diff_items = parse_usize(value, key)?,
                    "--min-line-diff-bytes" => min_line_diff_bytes = parse_u64(value, key)?,
                    _ => {}
                }
                index += 2;
                continue;
            }
            "--demangle=auto" => {
                demangle = DemangleMode::Auto;
                index += 1;
                continue;
            }
            "--demangle=on" => {
                demangle = DemangleMode::On;
                index += 1;
                continue;
            }
            "--demangle=off" => {
                demangle = DemangleMode::Off;
                index += 1;
                continue;
            }
            "--dwarf=auto" => {
                dwarf_mode = DwarfMode::Auto;
                index += 1;
                continue;
            }
            "--dwarf=on" => {
                dwarf_mode = DwarfMode::On;
                index += 1;
                continue;
            }
            "--dwarf=off" => {
                dwarf_mode = DwarfMode::Off;
                index += 1;
                continue;
            }
            "--toolchain" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --toolchain".to_string())?;
                toolchain = parse_toolchain(value)?;
                index += 2;
                continue;
            }
            "--source-lines" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --source-lines".to_string())?;
                source_lines = parse_source_lines_mode(value)?;
                index += 2;
                continue;
            }
            "--path-remap" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --path-remap".to_string())?;
                path_remaps.push(parse_path_remap(value)?);
                index += 2;
                continue;
            }
            "--elf" | "--map" | "--lds" | "--prev-elf" | "--prev-map" | "--out" | "--report-json" | "--rules" | "--ci-out" | "--source-root" => {
                let value = args.get(index + 1).ok_or_else(|| format!("missing value for {key}"))?;
                match key.as_str() {
                    "--elf" => elf = Some(PathBuf::from(value)),
                    "--map" => map = Some(PathBuf::from(value)),
                    "--lds" => lds = Some(PathBuf::from(value)),
                    "--prev-elf" => prev_elf = Some(PathBuf::from(value)),
                    "--prev-map" => prev_map = Some(PathBuf::from(value)),
                    "--out" => out = PathBuf::from(value),
                    "--report-json" => report_json = Some(PathBuf::from(value)),
                    "--rules" => rules = Some(PathBuf::from(value)),
                    "--ci-out" => ci_out = Some(PathBuf::from(value)),
                    "--source-root" => source_root = Some(PathBuf::from(value)),
                    _ => {}
                }
                index += 2;
                continue;
            }
            _ => return Err(format!("unknown option '{key}'")),
        }
    }

    let elf = elf.ok_or_else(|| "--elf is required".to_string())?;
    ensure_exists(&elf, "ELF")?;
    if let Some(path) = map.as_deref() {
        ensure_exists(path, "map")?;
    }
    if let Some(path) = prev_elf.as_deref() {
        ensure_exists(path, "previous ELF")?;
    }
    if let Some(path) = lds.as_deref() {
        ensure_exists(path, "linker script")?;
    }
    if let Some(path) = prev_map.as_deref() {
        ensure_exists(path, "previous map")?;
    }
    if let Some(path) = rules.as_deref() {
        ensure_exists(path, "rules")?;
    }

    Ok(Command::Analyze {
        elf,
        map,
        lds,
        prev_elf,
        prev_map,
        out,
        report_json,
        ci_summary,
        ci_format,
        ci_out,
        ci_source_summary,
        fail_on_warning,
        max_source_diff_items,
        min_line_diff_bytes,
        hide_unknown_source,
        thresholds,
        rules,
        demangle,
        toolchain,
        dwarf_mode,
        source_lines,
        source_root,
        path_remaps,
        fail_on_missing_dwarf,
        verbose,
    })
}

fn ensure_exists(path: &Path, label: &str) -> Result<(), String> {
    if path.exists() {
        Ok(())
    } else {
        Err(format!(
            "{label} file does not exist: {}. Check the path and confirm the build artifact was generated.",
            path.display()
        ))
    }
}

fn print_help() {
    println!("{}", help_text());
}

fn help_text() -> String {
    format!(
        "fwmap {VERSION}

fwmap analyze --elf <path> [--map <path>] [--lds <path>] [--prev-elf <path>] [--prev-map <path>] [--out <path>] [--report-json <path>] [--rules <path>] [--demangle=auto|on|off] [--toolchain <name>] [--dwarf=auto|on|off] [--source-lines <mode>] [--source-root <path>] [--path-remap <from=to>] [--fail-on-missing-dwarf] [--verbose]
fwmap history record --db <path> --elf <path> [--map <path>] [--lds <path>] [--rules <path>] [--demangle=auto|on|off] [--toolchain <name>] [--dwarf=auto|on|off] [--source-lines <mode>] [--source-root <path>] [--path-remap <from=to>] [--fail-on-missing-dwarf] [--meta key=value]
fwmap history list --db <path>
fwmap history show --db <path> --build <id>
fwmap history trend --db <path> --metric <rom|ram|warnings|region:NAME|section:NAME> [--last <n>]

Options:
  --elf       Input ELF file (required)
  --map       GNU ld map file
  --lds       GNU ld linker script
  --prev-elf  Previous ELF file for diff
  --prev-map  Previous map file for diff
  --out       Output HTML path (default: fwmap_report.html)
  --report-json Write JSON report to the given path
  --rules     Load TOML rule configuration from the given path
  --demangle=auto|on|off Control C++ symbol demangling
  --toolchain auto|gnu|lld|iar|armcc|keil Select or detect the map parser family
  --dwarf=auto|on|off Control DWARF line-table usage
  --source-lines off|files|functions|lines|all Control source-level aggregation
  --source-root Apply a root prefix to relative DWARF source paths
  --path-remap from=to Remap DWARF source path prefixes (repeatable)
  --fail-on-missing-dwarf Return an error when DWARF was requested but unavailable
  --ci-summary Print compact CI-friendly summary
  --ci-source-summary Include top growing source files, functions, and line ranges in CI output
  --ci-format text|markdown|json Select CI summary format
  --ci-out    Write CI summary to the given path
  --max-source-diff-items Limit source diff items shown in CI/HTML/JSON summaries
  --min-line-diff-bytes Minimum line hotspot diff size to display
  --hide-unknown-source Hide unknown-source diff rows from summaries
  --fail-on-warning Return non-zero if warnings are present
  --threshold-rom Percent threshold for ROM warnings
  --threshold-ram Percent threshold for RAM warnings
  --threshold-region name:percent threshold for a region warning
  --threshold-symbol-growth Bytes threshold for symbol growth warning
  --verbose   Print detailed warnings to the console
  --version   Show version
  --help      Show this help"
    )
}

fn parse_history_args(args: Vec<String>) -> Result<Command, String> {
    let sub = args.get(2).ok_or_else(|| format!("missing history subcommand\n\n{}", help_text()))?;
    match sub.as_str() {
        "record" => parse_history_record_args(args),
        "list" => parse_history_list_args(args),
        "show" => parse_history_show_args(args),
        "trend" => parse_history_trend_args(args),
        _ => Err(format!("unknown history subcommand '{}'\n\n{}", sub, help_text())),
    }
}

fn parse_history_record_args(args: Vec<String>) -> Result<Command, String> {
    let mut db = None;
    let mut elf = None;
    let mut map = None;
    let mut lds = None;
    let mut thresholds = ThresholdConfig::default();
    let mut rules = None;
    let mut demangle = DemangleMode::Auto;
    let mut toolchain = ToolchainSelection::Auto;
    let mut dwarf_mode = DwarfMode::Auto;
    let mut source_lines = SourceLinesMode::Off;
    let mut source_root = None;
    let mut path_remaps = Vec::new();
    let mut fail_on_missing_dwarf = false;
    let mut metadata = std::collections::BTreeMap::new();
    let mut index = 3usize;
    while index < args.len() {
        let key = &args[index];
        match key.as_str() {
            "--threshold-rom" | "--threshold-ram" | "--threshold-symbol-growth" | "--threshold-region" => {
                let value = args.get(index + 1).ok_or_else(|| format!("missing value for {key}"))?;
                match key.as_str() {
                    "--threshold-rom" => thresholds.rom_percent = parse_percent(value, key)?,
                    "--threshold-ram" => thresholds.ram_percent = parse_percent(value, key)?,
                    "--threshold-symbol-growth" => thresholds.symbol_growth_bytes = parse_u64(value, key)?,
                    "--threshold-region" => {
                        let (name, percent) = parse_region_threshold(value)?;
                        thresholds.region_percent.insert(name, percent);
                    }
                    _ => {}
                }
                index += 2;
            }
            "--demangle=auto" => {
                demangle = DemangleMode::Auto;
                index += 1;
            }
            "--demangle=on" => {
                demangle = DemangleMode::On;
                index += 1;
            }
            "--demangle=off" => {
                demangle = DemangleMode::Off;
                index += 1;
            }
            "--dwarf=auto" => {
                dwarf_mode = DwarfMode::Auto;
                index += 1;
            }
            "--dwarf=on" => {
                dwarf_mode = DwarfMode::On;
                index += 1;
            }
            "--dwarf=off" => {
                dwarf_mode = DwarfMode::Off;
                index += 1;
            }
            "--toolchain" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --toolchain".to_string())?;
                toolchain = parse_toolchain(value)?;
                index += 2;
            }
            "--source-lines" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --source-lines".to_string())?;
                source_lines = parse_source_lines_mode(value)?;
                index += 2;
            }
            "--path-remap" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --path-remap".to_string())?;
                path_remaps.push(parse_path_remap(value)?);
                index += 2;
            }
            "--fail-on-missing-dwarf" => {
                fail_on_missing_dwarf = true;
                index += 1;
            }
            "--meta" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --meta".to_string())?;
                let (k, v) = value
                    .split_once('=')
                    .ok_or_else(|| format!("invalid metadata '{value}', expected key=value"))?;
                metadata.insert(k.to_string(), v.to_string());
                index += 2;
            }
            "--db" | "--elf" | "--map" | "--lds" | "--rules" | "--source-root" => {
                let value = args.get(index + 1).ok_or_else(|| format!("missing value for {key}"))?;
                match key.as_str() {
                    "--db" => db = Some(PathBuf::from(value)),
                    "--elf" => elf = Some(PathBuf::from(value)),
                    "--map" => map = Some(PathBuf::from(value)),
                    "--lds" => lds = Some(PathBuf::from(value)),
                    "--rules" => rules = Some(PathBuf::from(value)),
                    "--source-root" => source_root = Some(PathBuf::from(value)),
                    _ => {}
                }
                index += 2;
            }
            _ => return Err(format!("unknown option '{key}'")),
        }
    }
    let db = db.ok_or_else(|| "--db is required".to_string())?;
    let elf = elf.ok_or_else(|| "--elf is required".to_string())?;
    ensure_exists(&elf, "ELF")?;
    if let Some(path) = map.as_deref() {
        ensure_exists(path, "map")?;
    }
    if let Some(path) = lds.as_deref() {
        ensure_exists(path, "linker script")?;
    }
    if let Some(path) = rules.as_deref() {
        ensure_exists(path, "rules")?;
    }
    Ok(Command::HistoryRecord {
        db,
        elf,
        map,
        lds,
        thresholds,
        rules,
        demangle,
        toolchain,
        dwarf_mode,
        source_lines,
        source_root,
        path_remaps,
        fail_on_missing_dwarf,
        metadata,
    })
}

fn parse_history_list_args(args: Vec<String>) -> Result<Command, String> {
    let db = parse_required_path_arg(&args[3..], "--db")?;
    Ok(Command::HistoryList { db })
}

fn parse_history_show_args(args: Vec<String>) -> Result<Command, String> {
    let db = parse_required_path_arg(&args[3..], "--db")?;
    let build = parse_required_i64_arg(&args[3..], "--build")?;
    Ok(Command::HistoryShow { db, build })
}

fn parse_history_trend_args(args: Vec<String>) -> Result<Command, String> {
    let db = parse_required_path_arg(&args[3..], "--db")?;
    let metric = parse_required_string_arg(&args[3..], "--metric")?;
    let last = parse_optional_usize_arg(&args[3..], "--last")?.unwrap_or(20);
    Ok(Command::HistoryTrend { db, metric, last })
}

fn parse_required_path_arg(args: &[String], key: &str) -> Result<PathBuf, String> {
    parse_required_string_arg(args, key).map(PathBuf::from)
}

fn parse_required_string_arg(args: &[String], key: &str) -> Result<String, String> {
    let index = args
        .iter()
        .position(|item| item == key)
        .ok_or_else(|| format!("{key} is required"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {key}"))
}

fn parse_required_i64_arg(args: &[String], key: &str) -> Result<i64, String> {
    parse_required_string_arg(args, key)?
        .parse::<i64>()
        .map_err(|_| format!("invalid integer for {key}"))
}

fn parse_optional_usize_arg(args: &[String], key: &str) -> Result<Option<usize>, String> {
    let Some(index) = args.iter().position(|item| item == key) else {
        return Ok(None);
    };
    let value = args
        .get(index + 1)
        .ok_or_else(|| format!("missing value for {key}"))?;
    value
        .parse::<usize>()
        .map(Some)
        .map_err(|_| format!("invalid integer for {key}"))
}

fn parse_percent(value: &str, key: &str) -> Result<f64, String> {
    let parsed = value.parse::<f64>().map_err(|_| format!("invalid percent for {key}: {value}"))?;
    if !(0.0..=100.0).contains(&parsed) {
        return Err(format!("percent for {key} must be between 0 and 100: {value}"));
    }
    Ok(parsed)
}

fn parse_u64(value: &str, key: &str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|_| format!("invalid integer for {key}: {value}"))
}

fn parse_usize(value: &str, key: &str) -> Result<usize, String> {
    value.parse::<usize>().map_err(|_| format!("invalid integer for {key}: {value}"))
}

fn parse_region_threshold(value: &str) -> Result<(String, f64), String> {
    let (name, percent) = value
        .split_once(':')
        .ok_or_else(|| format!("invalid region threshold '{value}', expected <name:percent>"))?;
    Ok((name.to_string(), parse_percent(percent, "--threshold-region")?))
}

fn parse_ci_format(value: &str) -> Result<CiFormat, String> {
    match value {
        "text" => Ok(CiFormat::Text),
        "markdown" => Ok(CiFormat::Markdown),
        "json" => Ok(CiFormat::Json),
        _ => Err(format!("invalid ci format '{value}', expected text|markdown|json")),
    }
}

fn parse_toolchain(value: &str) -> Result<ToolchainSelection, String> {
    match value {
        "auto" => Ok(ToolchainSelection::Auto),
        "gnu" => Ok(ToolchainSelection::Gnu),
        "lld" => Ok(ToolchainSelection::Lld),
        "iar" => Ok(ToolchainSelection::Iar),
        "armcc" => Ok(ToolchainSelection::Armcc),
        "keil" => Ok(ToolchainSelection::Keil),
        _ => Err(format!(
            "invalid toolchain '{value}', expected auto|gnu|lld|iar|armcc|keil"
        )),
    }
}

fn parse_source_lines_mode(value: &str) -> Result<SourceLinesMode, String> {
    match value {
        "off" => Ok(SourceLinesMode::Off),
        "files" => Ok(SourceLinesMode::Files),
        "functions" => Ok(SourceLinesMode::Functions),
        "lines" => Ok(SourceLinesMode::Lines),
        "all" => Ok(SourceLinesMode::All),
        _ => Err(format!("invalid source-lines mode '{value}', expected off|files|functions|lines|all")),
    }
}

fn parse_path_remap(value: &str) -> Result<(String, String), String> {
    value
        .split_once('=')
        .map(|(from, to)| (from.to_string(), to.to_string()))
        .ok_or_else(|| format!("invalid path remap '{value}', expected <from=to>"))
}

#[cfg(test)]
mod tests {
    use super::{parse_args, Command};
    use crate::model::{CiFormat, DemangleMode, DwarfMode, SourceLinesMode, ToolchainSelection};
    use std::path::PathBuf;

    #[test]
    fn help_without_args() {
        let cmd = parse_args(vec!["fwmap".to_string()]).unwrap();
        assert!(matches!(cmd, super::Command::Help));
    }

    #[test]
    fn parses_verbose_flag() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "analyze".to_string(),
            "--elf".to_string(),
            "Cargo.toml".to_string(),
            "--verbose".to_string(),
        ])
        .unwrap();
        assert!(matches!(cmd, Command::Analyze { verbose: true, .. }));
    }

    #[test]
    fn parses_lds_flag() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "analyze".to_string(),
            "--elf".to_string(),
            "Cargo.toml".to_string(),
            "--lds".to_string(),
            "README.md".to_string(),
        ])
        .unwrap();
        assert!(matches!(cmd, Command::Analyze { lds: Some(_), .. }));
    }

    #[test]
    fn parses_version_flag() {
        let cmd = parse_args(vec!["fwmap".to_string(), "--version".to_string()]).unwrap();
        assert!(matches!(cmd, Command::Version));
    }

    #[test]
    fn parses_json_and_threshold_flags() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "analyze".to_string(),
            "--elf".to_string(),
            "Cargo.toml".to_string(),
            "--report-json".to_string(),
            "out.json".to_string(),
            "--ci-format".to_string(),
            "markdown".to_string(),
            "--ci-out".to_string(),
            "ci.md".to_string(),
            "--ci-source-summary".to_string(),
            "--max-source-diff-items".to_string(),
            "7".to_string(),
            "--min-line-diff-bytes".to_string(),
            "64".to_string(),
            "--hide-unknown-source".to_string(),
            "--threshold-rom".to_string(),
            "90".to_string(),
            "--threshold-region".to_string(),
            "FLASH:92".to_string(),
            "--threshold-symbol-growth".to_string(),
            "8192".to_string(),
            "--rules".to_string(),
            "Cargo.toml".to_string(),
            "--demangle=on".to_string(),
            "--ci-summary".to_string(),
            "--fail-on-warning".to_string(),
        ])
        .unwrap();
        match cmd {
            Command::Analyze {
                report_json,
                ci_summary,
                ci_format,
                ci_out,
                ci_source_summary,
                rules,
                demangle,
                fail_on_warning,
                max_source_diff_items,
                min_line_diff_bytes,
                hide_unknown_source,
                thresholds,
                ..
            } => {
                assert_eq!(report_json.unwrap(), PathBuf::from("out.json"));
                assert!(matches!(ci_format, Some(CiFormat::Markdown)));
                assert_eq!(ci_out.unwrap(), PathBuf::from("ci.md"));
                assert_eq!(rules.unwrap(), PathBuf::from("Cargo.toml"));
                assert!(ci_summary);
                assert!(ci_source_summary);
                assert!(fail_on_warning);
                assert_eq!(max_source_diff_items, 7);
                assert_eq!(min_line_diff_bytes, 64);
                assert!(hide_unknown_source);
                assert!(matches!(demangle, DemangleMode::On));
                assert_eq!(thresholds.rom_percent, 90.0);
                assert_eq!(thresholds.region_percent.get("FLASH"), Some(&92.0));
                assert_eq!(thresholds.symbol_growth_bytes, 8192);
            }
            _ => panic!("expected analyze command"),
        }
    }

    #[test]
    fn parses_history_record_command() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "history".to_string(),
            "record".to_string(),
            "--db".to_string(),
            "history.db".to_string(),
            "--elf".to_string(),
            "Cargo.toml".to_string(),
            "--meta".to_string(),
            "commit=abc123".to_string(),
        ])
        .unwrap();
        assert!(matches!(cmd, Command::HistoryRecord { .. }));
    }

    #[test]
    fn parses_history_trend_command() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "history".to_string(),
            "trend".to_string(),
            "--db".to_string(),
            "history.db".to_string(),
            "--metric".to_string(),
            "rom".to_string(),
            "--last".to_string(),
            "5".to_string(),
        ])
        .unwrap();
        match cmd {
            Command::HistoryTrend { metric, last, .. } => {
                assert_eq!(metric, "rom");
                assert_eq!(last, 5);
            }
            _ => panic!("expected history trend command"),
        }
    }

    #[test]
    fn parses_toolchain_option() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "analyze".to_string(),
            "--elf".to_string(),
            "Cargo.toml".to_string(),
            "--toolchain".to_string(),
            "lld".to_string(),
        ])
        .unwrap();
        match cmd {
            Command::Analyze { toolchain, .. } => assert_eq!(toolchain, ToolchainSelection::Lld),
            _ => panic!("expected analyze command"),
        }
    }

    #[test]
    fn parses_dwarf_options() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "analyze".to_string(),
            "--elf".to_string(),
            "Cargo.toml".to_string(),
            "--dwarf=on".to_string(),
            "--source-lines".to_string(),
            "lines".to_string(),
            "--source-root".to_string(),
            "src".to_string(),
            "--path-remap".to_string(),
            "a=b".to_string(),
            "--fail-on-missing-dwarf".to_string(),
        ])
        .unwrap();
        match cmd {
            Command::Analyze {
                dwarf_mode,
                source_lines,
                source_root,
                path_remaps,
                fail_on_missing_dwarf,
                ..
            } => {
                assert_eq!(dwarf_mode, DwarfMode::On);
                assert_eq!(source_lines, SourceLinesMode::Lines);
                assert_eq!(source_root, Some(PathBuf::from("src")));
                assert_eq!(path_remaps, vec![("a".to_string(), "b".to_string())]);
                assert!(fail_on_missing_dwarf);
            }
            _ => panic!("expected analyze command"),
        }
    }
}
