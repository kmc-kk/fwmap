use std::path::{Path, PathBuf};

use crate::analyze::{analyze_paths, evaluate_warnings, AnalyzeOptions};
use crate::diff::{diff_results, top_increases};
use crate::demangle::display_name;
use crate::git::{CommitOrder, GitOptions};
use crate::history::{
    commit_timeline, list_builds, print_build_detail, print_build_list, print_commit_timeline, print_range_diff,
    print_trend, range_diff, record_build, show_build, trend_metric, write_commit_timeline_html, write_range_diff_html,
    HistoryRecordInput,
};
use crate::linkage::{explain_object, explain_section, explain_symbol, ExplainResult};
use crate::model::{
    CiFormat, CppGroupBy, DebuginfodMode, DemangleMode, DwarfMode, MapFormatSelection, SourceLinesMode,
    ThresholdConfig, ToolchainSelection, WarningLevel,
};
use crate::policy::{dump_effective_policy, evaluate_policy, load_policy_config, policy_warnings};
use crate::rule_config::{apply_threshold_overrides, load_rule_config};
use crate::render::{
    print_ci_summary, print_cli_summary, print_cpp_cli_summary, write_ci_summary, write_html_report, write_json_report,
};
use crate::sarif::{write_sarif_report, SarifOptions};

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
            policy,
            profile,
            demangle,
            toolchain,
            map_format,
            dwarf_mode,
            debug_file_dirs,
            debug_trace,
            git_enabled,
            git_repo,
            debuginfod,
            debuginfod_urls,
            debuginfod_cache_dir,
            source_lines,
            source_root,
            path_remaps,
            fail_on_missing_dwarf,
            policy_dump_effective,
            metadata,
        } => {
            let mut options = AnalyzeOptions {
                thresholds,
                demangle,
                custom_rules: Vec::new(),
                toolchain,
                map_format,
                dwarf_mode,
                debug_file_dirs,
                debug_trace,
                git: GitOptions {
                    enabled: git_enabled,
                    repo_path: git_repo,
                },
                debuginfod,
                debuginfod_urls,
                debuginfod_cache_dir,
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
            let mut analysis = analyze_paths(&elf, map.as_deref(), lds.as_deref(), &options)?;
            if let Some(policy_path) = policy.as_deref() {
                let config = load_policy_config(policy_path)?;
                let evaluation = evaluate_policy(&analysis, None, &config, profile.as_deref())?;
                if policy_dump_effective {
                    println!("{}", dump_effective_policy(&evaluation));
                }
                analysis.warnings.extend(policy_warnings(&evaluation));
                analysis.policy = Some(evaluation);
            }
            print_debug_trace(&analysis);
            let mut metadata = metadata;
            if let Some(profile) = profile.as_ref() {
                metadata.insert("build.profile".to_string(), profile.clone());
            }
            metadata.insert(
                "toolchain.id".to_string(),
                analysis
                    .toolchain
                    .detected
                    .map(|item| item.to_string())
                    .unwrap_or_else(|| analysis.toolchain.resolved.to_string()),
            );
            metadata.insert(
                "config.fingerprint".to_string(),
                format!(
                    "{}|{}|{}",
                    analysis.toolchain.linker_family,
                    analysis.toolchain.map_format,
                    analysis.debug_info.source_lines
                ),
            );
            let id = record_build(&db, HistoryRecordInput { analysis, metadata })?;
            println!("Recorded build #{id} into {}", db.display());
            Ok(0)
        }
        Command::HistoryList { db, limit, json } => {
            let mut items = list_builds(&db)?;
            items.truncate(limit);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&items).map_err(|err| format!("failed to serialize history list: {err}"))?
                );
            } else {
                print_build_list(&items);
            }
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
        Command::HistoryCommits {
            db,
            repo,
            branch,
            limit,
            profile,
            toolchain,
            target,
            order,
            json,
            html,
        } => {
            let report = commit_timeline(&db, repo.as_deref(), branch.as_deref(), limit, profile.as_deref(), toolchain.as_deref(), target.as_deref(), order)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|err| format!("failed to serialize commit timeline: {err}"))?
                );
            } else if let Some(path) = html.as_deref() {
                write_commit_timeline_html(path, &report)?;
                println!("HTML: {}", path.display());
            } else {
                print_commit_timeline(&report);
            }
            Ok(0)
        }
        Command::HistoryRange {
            db,
            repo,
            spec,
            order,
            include_changed_files,
            profile,
            toolchain,
            target,
            json,
            html,
        } => {
            let report = range_diff(
                &db,
                repo.as_deref(),
                &spec,
                order,
                include_changed_files,
                profile.as_deref(),
                toolchain.as_deref(),
                target.as_deref(),
            )?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|err| format!("failed to serialize range diff: {err}"))?
                );
            } else if let Some(path) = html.as_deref() {
                write_range_diff_html(path, &report)?;
                println!("HTML: {}", path.display());
            } else {
                print_range_diff(&report);
            }
            Ok(0)
        }
        Command::Explain {
            elf,
            map,
            lds,
            demangle,
            toolchain,
            map_format,
            dwarf_mode,
            debug_file_dirs,
            debug_trace,
            git_enabled,
            git_repo,
            debuginfod,
            debuginfod_urls,
            debuginfod_cache_dir,
            source_lines,
            source_root,
            path_remaps,
            fail_on_missing_dwarf,
            symbol,
            object,
            section,
        } => {
            let options = AnalyzeOptions {
                thresholds: ThresholdConfig::default(),
                demangle,
                custom_rules: Vec::new(),
                toolchain,
                map_format,
                dwarf_mode,
                debug_file_dirs,
                debug_trace,
                git: GitOptions {
                    enabled: git_enabled,
                    repo_path: git_repo.clone(),
                },
                debuginfod,
                debuginfod_urls,
                debuginfod_cache_dir,
                source_lines,
                source_root,
                path_remaps,
                fail_on_missing_dwarf,
            };
            let analysis = analyze_paths(&elf, map.as_deref(), lds.as_deref(), &options)?;
            print_debug_trace(&analysis);
            let explain = match (symbol.as_deref(), object.as_deref(), section.as_deref()) {
                (Some(value), None, None) => explain_symbol(&analysis, value),
                (None, Some(value), None) => explain_object(&analysis, value),
                (None, None, Some(value)) => explain_section(&analysis, value),
                _ => None,
            };
            match explain {
                Some(result) => {
                    print_explain_result(&result);
                    Ok(0)
                }
                None => Err("no explanation target was found in the current analysis".to_string()),
            }
        }
        Command::Analyze {
            elf,
            map,
            lds,
            prev_elf,
            prev_map,
            out,
            report_json,
            why_linked_top,
            sarif,
            sarif_base_uri,
            sarif_min_level,
            sarif_include_pass,
            sarif_tool_name,
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
            policy,
            profile,
            demangle,
            toolchain,
            map_format,
            dwarf_mode,
            debug_file_dirs,
            debug_trace,
            git_enabled,
            git_repo,
            debuginfod,
            debuginfod_urls,
            debuginfod_cache_dir,
            source_lines,
            source_root,
            path_remaps,
            fail_on_missing_dwarf,
            policy_dump_effective,
            verbose,
            cpp_view,
            group_by,
            save_history,
            history_db,
        } => {
            let mut options = AnalyzeOptions {
                thresholds,
                demangle,
                custom_rules: Vec::new(),
                toolchain,
                map_format,
                dwarf_mode,
                debug_file_dirs,
                debug_trace,
                git: GitOptions {
                    enabled: git_enabled,
                    repo_path: git_repo.clone(),
                },
                debuginfod,
                debuginfod_urls,
                debuginfod_cache_dir,
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
            let policy_config = if let Some(policy_path) = policy.as_deref() {
                Some(load_policy_config(policy_path)?)
            } else {
                None
            };

            let mut current = analyze_paths(&elf, map.as_deref(), lds.as_deref(), &options)?;
            print_debug_trace(&current);
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
            if let Some(config) = policy_config.as_ref() {
                let evaluation = evaluate_policy(&current, diff.as_ref(), config, profile.as_deref())?;
                if policy_dump_effective {
                    println!("{}", dump_effective_policy(&evaluation));
                }
                current.warnings.extend(policy_warnings(&evaluation));
                current.policy = Some(evaluation);
            }
            if save_history {
                let db = history_db.clone().unwrap_or_else(|| PathBuf::from("history.db"));
                let mut metadata = std::collections::BTreeMap::new();
                if let Some(profile) = profile.as_ref() {
                    metadata.insert("build.profile".to_string(), profile.clone());
                }
                metadata.insert(
                    "toolchain.id".to_string(),
                    current
                        .toolchain
                        .detected
                        .map(|item| item.to_string())
                        .unwrap_or_else(|| current.toolchain.resolved.to_string()),
                );
                metadata.insert(
                    "config.fingerprint".to_string(),
                    format!(
                        "{}|{}|{}",
                        current.toolchain.linker_family,
                        current.toolchain.map_format,
                        current.debug_info.source_lines
                    ),
                );
                let build_id = record_build(
                    &db,
                    HistoryRecordInput {
                        analysis: current.clone(),
                        metadata,
                    },
                )?;
                if !ci_summary {
                    println!("History: recorded build #{build_id} into {}", db.display());
                }
            }
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
                if cpp_view {
                    print_cpp_cli_summary(&current);
                }
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
                    print_grouped_cpp_diff(diff, group_by);
                    if why_linked_top > 0 {
                        let why = crate::linkage::explain_top_growth(&current, diff, 1);
                        if let Some(item) = why.top_symbols.first() {
                            println!("Why linked symbol: {}", item.summary);
                        }
                        if let Some(item) = why.top_objects.first() {
                            println!("Why linked object: {}", item.summary);
                        }
                    }
                }
            }
            let source_options = crate::render::SourceRenderOptions {
                enabled: ci_source_summary,
                max_diff_items: max_source_diff_items,
                min_line_diff_bytes,
                hide_unknown_source,
            };
            write_html_report(&out, &current, diff.as_ref(), source_options, why_linked_top)?;
            if let Some(path) = report_json.as_deref() {
                write_json_report(path, &current, diff.as_ref(), &options.thresholds, source_options, why_linked_top)?;
                if !ci_summary {
                    println!("JSON: {}", path.display());
                }
            }
            if let Some(path) = sarif.as_deref() {
                write_sarif_report(
                    path,
                    &current,
                    &SarifOptions {
                        base_uri: sarif_base_uri,
                        min_level: sarif_min_level,
                        include_pass: sarif_include_pass,
                        tool_name: sarif_tool_name,
                    },
                )?;
                if !ci_summary {
                    println!("SARIF: {}", path.display());
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
        policy: Option<PathBuf>,
        profile: Option<String>,
        demangle: DemangleMode,
        toolchain: ToolchainSelection,
        map_format: MapFormatSelection,
        dwarf_mode: DwarfMode,
        debug_file_dirs: Vec<PathBuf>,
        debug_trace: bool,
        git_enabled: bool,
        git_repo: Option<PathBuf>,
        debuginfod: DebuginfodMode,
        debuginfod_urls: Vec<String>,
        debuginfod_cache_dir: Option<PathBuf>,
        source_lines: SourceLinesMode,
        source_root: Option<PathBuf>,
        path_remaps: Vec<(String, String)>,
        fail_on_missing_dwarf: bool,
        policy_dump_effective: bool,
        metadata: std::collections::BTreeMap<String, String>,
    },
    HistoryList { db: PathBuf, limit: usize, json: bool },
    HistoryShow {
        db: PathBuf,
        build: i64,
    },
    HistoryTrend {
        db: PathBuf,
        metric: String,
        last: usize,
    },
    HistoryCommits {
        db: PathBuf,
        repo: Option<PathBuf>,
        branch: Option<String>,
        limit: usize,
        profile: Option<String>,
        toolchain: Option<String>,
        target: Option<String>,
        order: CommitOrder,
        json: bool,
        html: Option<PathBuf>,
    },
    HistoryRange {
        db: PathBuf,
        repo: Option<PathBuf>,
        spec: String,
        order: CommitOrder,
        include_changed_files: bool,
        profile: Option<String>,
        toolchain: Option<String>,
        target: Option<String>,
        json: bool,
        html: Option<PathBuf>,
    },
    Explain {
        elf: PathBuf,
        map: Option<PathBuf>,
        lds: Option<PathBuf>,
        demangle: DemangleMode,
        toolchain: ToolchainSelection,
        map_format: MapFormatSelection,
        dwarf_mode: DwarfMode,
        debug_file_dirs: Vec<PathBuf>,
        debug_trace: bool,
        git_enabled: bool,
        git_repo: Option<PathBuf>,
        debuginfod: DebuginfodMode,
        debuginfod_urls: Vec<String>,
        debuginfod_cache_dir: Option<PathBuf>,
        source_lines: SourceLinesMode,
        source_root: Option<PathBuf>,
        path_remaps: Vec<(String, String)>,
        fail_on_missing_dwarf: bool,
        symbol: Option<String>,
        object: Option<String>,
        section: Option<String>,
    },
    Analyze {
        elf: PathBuf,
        map: Option<PathBuf>,
        lds: Option<PathBuf>,
        prev_elf: Option<PathBuf>,
        prev_map: Option<PathBuf>,
        out: PathBuf,
        report_json: Option<PathBuf>,
        why_linked_top: usize,
        sarif: Option<PathBuf>,
        sarif_base_uri: Option<String>,
        sarif_min_level: WarningLevel,
        sarif_include_pass: bool,
        sarif_tool_name: String,
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
        policy: Option<PathBuf>,
        profile: Option<String>,
        demangle: DemangleMode,
        toolchain: ToolchainSelection,
        map_format: MapFormatSelection,
        dwarf_mode: DwarfMode,
        debug_file_dirs: Vec<PathBuf>,
        debug_trace: bool,
        git_enabled: bool,
        git_repo: Option<PathBuf>,
        debuginfod: DebuginfodMode,
        debuginfod_urls: Vec<String>,
        debuginfod_cache_dir: Option<PathBuf>,
        source_lines: SourceLinesMode,
        source_root: Option<PathBuf>,
        path_remaps: Vec<(String, String)>,
        fail_on_missing_dwarf: bool,
        policy_dump_effective: bool,
        verbose: bool,
        cpp_view: bool,
        group_by: CppGroupBy,
        save_history: bool,
        history_db: Option<PathBuf>,
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
    if args[1] == "explain" {
        return parse_explain_args(args);
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
    let mut why_linked_top = 5usize;
    let mut sarif = None;
    let mut sarif_base_uri = None;
    let mut sarif_min_level = WarningLevel::Warn;
    let mut sarif_include_pass = false;
    let mut sarif_tool_name = env!("CARGO_PKG_NAME").to_string();
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
    let mut policy = None;
    let mut profile = None;
    let mut demangle = DemangleMode::Auto;
    let mut toolchain = ToolchainSelection::Auto;
    let mut map_format = MapFormatSelection::Auto;
    let mut dwarf_mode = DwarfMode::Auto;
    let mut debug_file_dirs = Vec::new();
    let mut debug_trace = false;
    let mut git_enabled = true;
    let mut git_repo = None;
    let mut debuginfod = DebuginfodMode::Off;
    let mut debuginfod_urls = Vec::new();
    let mut debuginfod_cache_dir = None;
    let mut source_lines = SourceLinesMode::Off;
    let mut source_root = None;
    let mut path_remaps = Vec::new();
    let mut fail_on_missing_dwarf = false;
    let mut policy_dump_effective = false;
    let mut verbose = false;
    let mut cpp_view = false;
    let mut group_by = CppGroupBy::Symbol;
    let mut save_history = false;
    let mut history_db = None;
    let mut index = 2usize;
    while index < args.len() {
        let key = &args[index];
        match key.as_str() {
            "--verbose" => {
                verbose = true;
                index += 1;
                continue;
            }
            "--cpp-view" => {
                cpp_view = true;
                index += 1;
                continue;
            }
            "--group-by" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --group-by".to_string())?;
                group_by = parse_cpp_group_by(value)?;
                index += 2;
                continue;
            }
            "--ci-summary" => {
                ci_summary = true;
                index += 1;
                continue;
            }
            "--save-history" => {
                save_history = true;
                index += 1;
                continue;
            }
            "--no-git" => {
                git_enabled = false;
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
            "--debug-trace" => {
                debug_trace = true;
                index += 1;
                continue;
            }
            "--policy-dump-effective" => {
                policy_dump_effective = true;
                index += 1;
                continue;
            }
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
            "--threshold-rom" | "--threshold-ram" | "--threshold-symbol-growth" | "--threshold-region" | "--max-source-diff-items" | "--min-line-diff-bytes" | "--sarif-min-level" | "--sarif-include-pass" | "--sarif-tool-name" | "--why-linked-top" => {
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
                    "--sarif-min-level" => sarif_min_level = parse_warning_level(value, key)?,
                    "--sarif-include-pass" => sarif_include_pass = parse_bool(value, key)?,
                    "--sarif-tool-name" => sarif_tool_name = value.to_string(),
                    "--why-linked-top" => why_linked_top = parse_usize(value, key)?,
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
            "--debuginfod=auto" => {
                debuginfod = DebuginfodMode::Auto;
                index += 1;
                continue;
            }
            "--debuginfod=on" => {
                debuginfod = DebuginfodMode::On;
                index += 1;
                continue;
            }
            "--debuginfod=off" => {
                debuginfod = DebuginfodMode::Off;
                index += 1;
                continue;
            }
            "--toolchain" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --toolchain".to_string())?;
                toolchain = parse_toolchain(value)?;
                index += 2;
                continue;
            }
            "--map-format" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --map-format".to_string())?;
                map_format = parse_map_format(value)?;
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
            "--elf" | "--map" | "--lds" | "--prev-elf" | "--prev-map" | "--out" | "--report-json" | "--sarif" | "--sarif-base-uri" | "--rules" | "--policy" | "--profile" | "--ci-out" | "--source-root" | "--debug-file-dir" | "--debuginfod-url" | "--debuginfod-cache-dir" | "--git-repo" | "--history-db" => {
                let value = args.get(index + 1).ok_or_else(|| format!("missing value for {key}"))?;
                match key.as_str() {
                    "--elf" => elf = Some(PathBuf::from(value)),
                    "--map" => map = Some(PathBuf::from(value)),
                    "--lds" => lds = Some(PathBuf::from(value)),
                    "--prev-elf" => prev_elf = Some(PathBuf::from(value)),
                    "--prev-map" => prev_map = Some(PathBuf::from(value)),
                    "--out" => out = PathBuf::from(value),
                    "--report-json" => report_json = Some(PathBuf::from(value)),
                    "--sarif" => sarif = Some(PathBuf::from(value)),
                    "--sarif-base-uri" => sarif_base_uri = Some(value.to_string()),
                    "--rules" => rules = Some(PathBuf::from(value)),
                    "--policy" => policy = Some(PathBuf::from(value)),
                    "--profile" => profile = Some(value.to_string()),
                    "--ci-out" => ci_out = Some(PathBuf::from(value)),
                    "--source-root" => source_root = Some(PathBuf::from(value)),
                    "--debug-file-dir" => debug_file_dirs.push(PathBuf::from(value)),
                    "--debuginfod-url" => debuginfod_urls.push(value.to_string()),
                    "--debuginfod-cache-dir" => debuginfod_cache_dir = Some(PathBuf::from(value)),
                    "--git-repo" => git_repo = Some(PathBuf::from(value)),
                    "--history-db" => history_db = Some(PathBuf::from(value)),
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
    if let Some(path) = policy.as_deref() {
        ensure_exists(path, "policy")?;
    }

    Ok(Command::Analyze {
        elf,
        map,
        lds,
        prev_elf,
        prev_map,
        out,
        report_json,
        why_linked_top,
        sarif,
        sarif_base_uri,
        sarif_min_level,
        sarif_include_pass,
        sarif_tool_name,
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
        policy,
        profile,
        demangle,
        toolchain,
        map_format,
        dwarf_mode,
        debug_file_dirs,
        debug_trace,
        git_enabled,
        git_repo,
        debuginfod,
        debuginfod_urls,
        debuginfod_cache_dir,
        source_lines,
        source_root,
        path_remaps,
        fail_on_missing_dwarf,
        policy_dump_effective,
        verbose,
        cpp_view,
        group_by,
        save_history,
        history_db,
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

fn print_debug_trace(result: &crate::model::AnalysisResult) {
    if result.debug_artifact.resolution_steps.is_empty() {
        return;
    }
    println!("Debug artifact trace:");
    for step in &result.debug_artifact.resolution_steps {
        println!("  {step}");
    }
}

fn print_explain_result(result: &ExplainResult) {
    println!("Target: {}", result.target);
    println!("Confidence: {:?}", result.confidence);
    println!("{}", result.summary);
    if !result.evidence.is_empty() {
        println!("Evidence:");
        for item in &result.evidence {
            println!("  [{:?}:{}] {}", item.kind, item.source, item.detail);
        }
    }
}

fn help_text() -> String {
    format!(
        "fwmap {VERSION}

fwmap analyze --elf <path> [--map <path>] [--lds <path>] [--prev-elf <path>] [--prev-map <path>] [--out <path>] [--report-json <path>] [--why-linked-top <n>] [--sarif <path>] [--sarif-base-uri <uri>] [--sarif-min-level <level>] [--sarif-include-pass <bool>] [--sarif-tool-name <name>] [--rules <path>] [--policy <path>] [--profile <name>] [--policy-dump-effective] [--demangle=auto|on|off] [--toolchain <name>] [--map-format <name>] [--dwarf=auto|on|off] [--debug-file-dir <path>] [--debug-trace] [--git-repo <path>] [--no-git] [--save-history] [--history-db <path>] [--debuginfod=auto|on|off] [--debuginfod-url <url>] [--debuginfod-cache-dir <path>] [--source-lines <mode>] [--source-root <path>] [--path-remap <from=to>] [--fail-on-missing-dwarf] [--cpp-view] [--group-by <mode>] [--verbose]
fwmap explain --elf <path> [--map <path>] [--lds <path>] [--demangle=auto|on|off] [--toolchain <name>] [--map-format <name>] [--dwarf=auto|on|off] [--debug-file-dir <path>] [--debug-trace] [--git-repo <path>] [--no-git] [--debuginfod=auto|on|off] [--debuginfod-url <url>] [--debuginfod-cache-dir <path>] [--source-lines <mode>] [--source-root <path>] [--path-remap <from=to>] [--fail-on-missing-dwarf] (--symbol <name> | --object <name> | --section <name>)
fwmap history record --db <path> --elf <path> [--map <path>] [--lds <path>] [--rules <path>] [--policy <path>] [--profile <name>] [--policy-dump-effective] [--demangle=auto|on|off] [--toolchain <name>] [--map-format <name>] [--dwarf=auto|on|off] [--debug-file-dir <path>] [--debug-trace] [--git-repo <path>] [--no-git] [--debuginfod=auto|on|off] [--debuginfod-url <url>] [--debuginfod-cache-dir <path>] [--source-lines <mode>] [--source-root <path>] [--path-remap <from=to>] [--fail-on-missing-dwarf] [--meta key=value]
fwmap history list --db <path> [--limit <n>] [--json]
fwmap history show --db <path> --build <id>
fwmap history trend --db <path> --metric <rom|ram|warnings|unknown_source|region:NAME|section:NAME|source:PATH|function:KEY|object:PATH|archive-member:ARCHIVE(MEMBER)|directory:PATH> [--last <n>]
fwmap history commits [--db <path>] [--repo <path>] [--branch <name>] [--limit <n>] [--profile <name>] [--toolchain <id>] [--target <id>] [--order <timestamp|ancestry>] [--json] [--html <path>]
fwmap history range <A..B|A...B> [--db <path>] [--repo <path>] [--profile <name>] [--toolchain <id>] [--target <id>] [--order <timestamp|ancestry>] [--include-changed-files] [--json] [--html <path>]

Options:
  --elf       Input ELF file (required)
  --map       GNU ld map file
  --lds       GNU ld linker script
  --prev-elf  Previous ELF file for diff
  --prev-map  Previous map file for diff
  --out       Output HTML path (default: fwmap_report.html)
  --report-json Write JSON report to the given path
  --why-linked-top Number of why-linked explanations shown in report output
  --sarif     Write SARIF 2.1.0 report to the given path
  --sarif-base-uri Base URI used for repo-relative SARIF locations
  --sarif-min-level info|warn|error Minimum warning level included in SARIF output
  --sarif-include-pass true|false Include pass metadata in SARIF properties
  --sarif-tool-name Override the SARIF driver name
  --rules     Load TOML rule configuration from the given path
  --policy    Load TOML policy configuration version 2
  --profile   Select a policy profile name
  --policy-dump-effective Print the selected effective policy summary
  --demangle=auto|on|off Control C++ symbol demangling
  --toolchain auto|gnu|lld|iar|armcc|keil Select or detect the map parser family
  --map-format auto|gnu|lld-native Select or detect the map text format
  --dwarf=auto|on|off Control DWARF line-table usage
  --debug-file-dir Search this directory for separate debug files (repeatable)
  --debug-trace Print debug artifact resolution steps
  --git-repo  Probe Git metadata from the given repository path
  --no-git    Disable Git metadata collection
  --save-history Save analyze output into a history database
  --history-db Override the history database path used with --save-history
  --debuginfod=auto|on|off Control debuginfod fallback behavior
  --debuginfod-url Add a debuginfod base URL (repeatable)
  --debuginfod-cache-dir Directory used for debuginfod cache metadata
  --symbol    Explain why a symbol is linked
  --object    Explain why an object or archive member is linked
  --section   Explain why a section is linked or placed
  --source-lines off|files|functions|lines|all Control source-level aggregation
  --source-root Apply a root prefix to relative DWARF source paths
  --path-remap from=to Remap DWARF source path prefixes (repeatable)
  --fail-on-missing-dwarf Return an error when DWARF was requested but unavailable
  --cpp-view  Print C++ aggregate summaries in CLI output
  --group-by symbol|cpp-template-family|cpp-class|cpp-runtime-overhead|cpp-lambda-group Select the top diff grouping shown in CLI output
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
  --help      Show this help

History metrics:
  rom | ram | warnings | unknown_source
  region:FLASH | section:.text | source:src/main.cpp
  function:src/main.cpp::_ZN3app4mainEv | directory:src/app"
    )
}

fn parse_history_args(args: Vec<String>) -> Result<Command, String> {
    let sub = args.get(2).ok_or_else(|| format!("missing history subcommand\n\n{}", help_text()))?;
    match sub.as_str() {
        "record" => parse_history_record_args(args),
        "list" => parse_history_list_args(args),
        "show" => parse_history_show_args(args),
        "trend" => parse_history_trend_args(args),
        "commits" => parse_history_commits_args(args),
        "range" => parse_history_range_args(args),
        _ => Err(format!("unknown history subcommand '{}'\n\n{}", sub, help_text())),
    }
}

fn parse_explain_args(args: Vec<String>) -> Result<Command, String> {
    let mut elf = None;
    let mut map = None;
    let mut lds = None;
    let mut demangle = DemangleMode::Auto;
    let mut toolchain = ToolchainSelection::Auto;
    let mut map_format = MapFormatSelection::Auto;
    let mut dwarf_mode = DwarfMode::Auto;
    let mut debug_file_dirs = Vec::new();
    let mut debug_trace = false;
    let mut git_enabled = true;
    let mut git_repo = None;
    let mut debuginfod = DebuginfodMode::Off;
    let mut debuginfod_urls = Vec::new();
    let mut debuginfod_cache_dir = None;
    let mut source_lines = SourceLinesMode::Off;
    let mut source_root = None;
    let mut path_remaps = Vec::new();
    let mut fail_on_missing_dwarf = false;
    let mut symbol = None;
    let mut object = None;
    let mut section = None;
    let mut index = 2usize;
    while index < args.len() {
        let key = &args[index];
        match key.as_str() {
            "--demangle=auto" => demangle = DemangleMode::Auto,
            "--demangle=on" => demangle = DemangleMode::On,
            "--demangle=off" => demangle = DemangleMode::Off,
            "--dwarf=auto" => dwarf_mode = DwarfMode::Auto,
            "--dwarf=on" => dwarf_mode = DwarfMode::On,
            "--dwarf=off" => dwarf_mode = DwarfMode::Off,
            "--debug-trace" => debug_trace = true,
            "--no-git" => git_enabled = false,
            "--debuginfod=auto" => debuginfod = DebuginfodMode::Auto,
            "--debuginfod=on" => debuginfod = DebuginfodMode::On,
            "--debuginfod=off" => debuginfod = DebuginfodMode::Off,
            "--fail-on-missing-dwarf" => fail_on_missing_dwarf = true,
            "--toolchain" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --toolchain".to_string())?;
                toolchain = parse_toolchain(value)?;
                index += 2;
                continue;
            }
            "--map-format" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --map-format".to_string())?;
                map_format = parse_map_format(value)?;
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
            "--elf" | "--map" | "--lds" | "--source-root" | "--debug-file-dir" | "--debuginfod-url" | "--debuginfod-cache-dir" | "--git-repo" | "--symbol" | "--object" | "--section" => {
                let value = args.get(index + 1).ok_or_else(|| format!("missing value for {key}"))?;
                match key.as_str() {
                    "--elf" => elf = Some(PathBuf::from(value)),
                    "--map" => map = Some(PathBuf::from(value)),
                    "--lds" => lds = Some(PathBuf::from(value)),
                    "--source-root" => source_root = Some(PathBuf::from(value)),
                    "--debug-file-dir" => debug_file_dirs.push(PathBuf::from(value)),
                    "--debuginfod-url" => debuginfod_urls.push(value.to_string()),
                    "--debuginfod-cache-dir" => debuginfod_cache_dir = Some(PathBuf::from(value)),
                    "--git-repo" => git_repo = Some(PathBuf::from(value)),
                    "--symbol" => symbol = Some(value.to_string()),
                    "--object" => object = Some(value.to_string()),
                    "--section" => section = Some(value.to_string()),
                    _ => {}
                }
                index += 2;
                continue;
            }
            "--help" | "-h" => return Ok(Command::Help),
            _ => return Err(format!("unknown option '{key}'")),
        }
        index += 1;
    }

    let target_count = usize::from(symbol.is_some()) + usize::from(object.is_some()) + usize::from(section.is_some());
    if target_count != 1 {
        return Err("explain requires exactly one of --symbol, --object, or --section".to_string());
    }

    let elf = elf.ok_or_else(|| "--elf is required".to_string())?;
    ensure_exists(&elf, "ELF")?;
    if let Some(path) = map.as_deref() {
        ensure_exists(path, "map")?;
    }
    if let Some(path) = lds.as_deref() {
        ensure_exists(path, "linker script")?;
    }

    Ok(Command::Explain {
        elf,
        map,
        lds,
        demangle,
        toolchain,
        map_format,
        dwarf_mode,
        debug_file_dirs,
        debug_trace,
        git_enabled,
        git_repo,
        debuginfod,
        debuginfod_urls,
        debuginfod_cache_dir,
        source_lines,
        source_root,
        path_remaps,
        fail_on_missing_dwarf,
        symbol,
        object,
        section,
    })
}

fn parse_history_record_args(args: Vec<String>) -> Result<Command, String> {
    let mut db = None;
    let mut elf = None;
    let mut map = None;
    let mut lds = None;
    let mut thresholds = ThresholdConfig::default();
    let mut rules = None;
    let mut policy = None;
    let mut profile = None;
    let mut demangle = DemangleMode::Auto;
    let mut toolchain = ToolchainSelection::Auto;
    let mut map_format = MapFormatSelection::Auto;
    let mut dwarf_mode = DwarfMode::Auto;
    let mut debug_file_dirs = Vec::new();
    let mut debug_trace = false;
    let mut git_enabled = true;
    let mut git_repo = None;
    let mut debuginfod = DebuginfodMode::Off;
    let mut debuginfod_urls = Vec::new();
    let mut debuginfod_cache_dir = None;
    let mut source_lines = SourceLinesMode::Off;
    let mut source_root = None;
    let mut path_remaps = Vec::new();
    let mut fail_on_missing_dwarf = false;
    let mut policy_dump_effective = false;
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
            "--debug-trace" => {
                debug_trace = true;
                index += 1;
            }
            "--no-git" => {
                git_enabled = false;
                index += 1;
            }
            "--debuginfod=auto" => {
                debuginfod = DebuginfodMode::Auto;
                index += 1;
            }
            "--debuginfod=on" => {
                debuginfod = DebuginfodMode::On;
                index += 1;
            }
            "--debuginfod=off" => {
                debuginfod = DebuginfodMode::Off;
                index += 1;
            }
            "--toolchain" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --toolchain".to_string())?;
                toolchain = parse_toolchain(value)?;
                index += 2;
            }
            "--map-format" => {
                let value = args.get(index + 1).ok_or_else(|| "missing value for --map-format".to_string())?;
                map_format = parse_map_format(value)?;
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
            "--policy-dump-effective" => {
                policy_dump_effective = true;
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
            "--db" | "--elf" | "--map" | "--lds" | "--rules" | "--policy" | "--profile" | "--source-root" | "--debug-file-dir" | "--debuginfod-url" | "--debuginfod-cache-dir" | "--git-repo" => {
                let value = args.get(index + 1).ok_or_else(|| format!("missing value for {key}"))?;
                match key.as_str() {
                    "--db" => db = Some(PathBuf::from(value)),
                    "--elf" => elf = Some(PathBuf::from(value)),
                    "--map" => map = Some(PathBuf::from(value)),
                    "--lds" => lds = Some(PathBuf::from(value)),
                    "--rules" => rules = Some(PathBuf::from(value)),
                    "--policy" => policy = Some(PathBuf::from(value)),
                    "--profile" => profile = Some(value.to_string()),
                    "--source-root" => source_root = Some(PathBuf::from(value)),
                    "--debug-file-dir" => debug_file_dirs.push(PathBuf::from(value)),
                    "--debuginfod-url" => debuginfod_urls.push(value.to_string()),
                    "--debuginfod-cache-dir" => debuginfod_cache_dir = Some(PathBuf::from(value)),
                    "--git-repo" => git_repo = Some(PathBuf::from(value)),
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
    if let Some(path) = policy.as_deref() {
        ensure_exists(path, "policy")?;
    }
    Ok(Command::HistoryRecord {
        db,
        elf,
        map,
        lds,
        thresholds,
        rules,
        policy,
        profile,
        demangle,
        toolchain,
        map_format,
        dwarf_mode,
        debug_file_dirs,
        debug_trace,
        git_enabled,
        git_repo,
        debuginfod,
        debuginfod_urls,
        debuginfod_cache_dir,
        source_lines,
        source_root,
        path_remaps,
        fail_on_missing_dwarf,
        policy_dump_effective,
        metadata,
    })
}

fn parse_history_list_args(args: Vec<String>) -> Result<Command, String> {
    let db = parse_required_path_arg(&args[3..], "--db")?;
    let limit = parse_optional_usize_arg(&args[3..], "--limit")?.unwrap_or(20);
    let json = args[3..].iter().any(|item| item == "--json");
    Ok(Command::HistoryList { db, limit, json })
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

fn parse_history_commits_args(args: Vec<String>) -> Result<Command, String> {
    let db = parse_optional_path_arg(&args[3..], "--db")?.unwrap_or_else(|| PathBuf::from("history.db"));
    let repo = parse_optional_path_arg(&args[3..], "--repo")?;
    let branch = parse_optional_string_arg(&args[3..], "--branch")?;
    let limit = parse_optional_usize_arg(&args[3..], "--limit")?.unwrap_or(100);
    let profile = parse_optional_string_arg(&args[3..], "--profile")?;
    let toolchain = parse_optional_string_arg(&args[3..], "--toolchain")?;
    let target = parse_optional_string_arg(&args[3..], "--target")?;
    let order = parse_optional_string_arg(&args[3..], "--order")?
        .as_deref()
        .map(parse_commit_order)
        .transpose()?
        .unwrap_or(CommitOrder::Timestamp);
    let json = args[3..].iter().any(|item| item == "--json");
    let html = parse_optional_path_arg(&args[3..], "--html")?;
    Ok(Command::HistoryCommits {
        db,
        repo,
        branch,
        limit,
        profile,
        toolchain,
        target,
        order,
        json,
        html,
    })
}

fn parse_history_range_args(args: Vec<String>) -> Result<Command, String> {
    let db = parse_optional_path_arg(&args[3..], "--db")?.unwrap_or_else(|| PathBuf::from("history.db"));
    let repo = parse_optional_path_arg(&args[3..], "--repo")?;
    let spec = args
        .get(3)
        .filter(|item| !item.starts_with("--"))
        .cloned()
        .or_else(|| {
            let base = parse_optional_string_arg(&args[3..], "--base").ok().flatten()?;
            let head = parse_optional_string_arg(&args[3..], "--head").ok().flatten()?;
            Some(format!("{base}...{head}"))
        })
        .or_else(|| {
            let from = parse_optional_string_arg(&args[3..], "--from").ok().flatten()?;
            let to = parse_optional_string_arg(&args[3..], "--to").ok().flatten()?;
            Some(format!("{from}..{to}"))
        })
        .ok_or_else(|| "history range requires <A..B>, <A...B>, --base/--head, or --from/--to".to_string())?;
    let profile = parse_optional_string_arg(&args[3..], "--profile")?;
    let toolchain = parse_optional_string_arg(&args[3..], "--toolchain")?;
    let target = parse_optional_string_arg(&args[3..], "--target")?;
    let order = parse_optional_string_arg(&args[3..], "--order")?
        .as_deref()
        .map(parse_commit_order)
        .transpose()?
        .unwrap_or(CommitOrder::Timestamp);
    let include_changed_files = args[3..].iter().any(|item| item == "--include-changed-files");
    let json = args[3..].iter().any(|item| item == "--json");
    let html = parse_optional_path_arg(&args[3..], "--html")?;
    Ok(Command::HistoryRange {
        db,
        repo,
        spec,
        order,
        include_changed_files,
        profile,
        toolchain,
        target,
        json,
        html,
    })
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

fn parse_optional_string_arg(args: &[String], key: &str) -> Result<Option<String>, String> {
    let Some(index) = args.iter().position(|item| item == key) else {
        return Ok(None);
    };
    args.get(index + 1)
        .cloned()
        .map(Some)
        .ok_or_else(|| format!("missing value for {key}"))
}

fn parse_optional_path_arg(args: &[String], key: &str) -> Result<Option<PathBuf>, String> {
    parse_optional_string_arg(args, key).map(|item| item.map(PathBuf::from))
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

fn parse_map_format(value: &str) -> Result<MapFormatSelection, String> {
    match value {
        "auto" => Ok(MapFormatSelection::Auto),
        "gnu" => Ok(MapFormatSelection::Gnu),
        "lld-native" => Ok(MapFormatSelection::LldNative),
        _ => Err(format!("invalid map format '{value}', expected auto|gnu|lld-native")),
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

fn parse_warning_level(value: &str, key: &str) -> Result<WarningLevel, String> {
    match value {
        "info" | "note" => Ok(WarningLevel::Info),
        "warn" | "warning" => Ok(WarningLevel::Warn),
        "error" => Ok(WarningLevel::Error),
        _ => Err(format!("invalid value for {key}: '{value}', expected info|warn|error")),
    }
}

fn parse_bool(value: &str, key: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("invalid value for {key}: '{value}', expected true|false")),
    }
}

fn parse_path_remap(value: &str) -> Result<(String, String), String> {
    value
        .split_once('=')
        .map(|(from, to)| (from.to_string(), to.to_string()))
        .ok_or_else(|| format!("invalid path remap '{value}', expected <from=to>"))
}

fn parse_cpp_group_by(value: &str) -> Result<CppGroupBy, String> {
    match value {
        "symbol" => Ok(CppGroupBy::Symbol),
        "cpp-template-family" => Ok(CppGroupBy::CppTemplateFamily),
        "cpp-class" => Ok(CppGroupBy::CppClass),
        "cpp-runtime-overhead" => Ok(CppGroupBy::CppRuntimeOverhead),
        "cpp-lambda-group" => Ok(CppGroupBy::CppLambdaGroup),
        _ => Err(format!(
            "invalid value for --group-by: '{value}', expected symbol|cpp-template-family|cpp-class|cpp-runtime-overhead|cpp-lambda-group"
        )),
    }
}

fn parse_commit_order(value: &str) -> Result<CommitOrder, String> {
    match value {
        "timestamp" => Ok(CommitOrder::Timestamp),
        "ancestry" => Ok(CommitOrder::Ancestry),
        _ => Err(format!("invalid value for --order: '{value}', expected timestamp|ancestry")),
    }
}

fn print_grouped_cpp_diff(diff: &crate::model::DiffResult, group_by: CppGroupBy) {
    let (label, entries) = match group_by {
        CppGroupBy::Symbol => return,
        CppGroupBy::CppTemplateFamily => ("Top growth template family", &diff.cpp_template_family_diffs),
        CppGroupBy::CppClass => ("Top growth class", &diff.cpp_class_diffs),
        CppGroupBy::CppRuntimeOverhead => ("Top runtime overhead", &diff.cpp_runtime_overhead_diffs),
        CppGroupBy::CppLambdaGroup => ("Top lambda group", &diff.cpp_lambda_group_diffs),
    };
    if let Some(entry) = top_increases(entries, 1).first() {
        println!("{label}: {} ({:+})", entry.name, entry.delta);
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_args, Command};
    use crate::git::CommitOrder;
    use crate::model::{
        CiFormat, CppGroupBy, DemangleMode, DwarfMode, MapFormatSelection, SourceLinesMode, ToolchainSelection,
        WarningLevel,
    };
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
    fn parses_cpp_view_flag() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "analyze".to_string(),
            "--elf".to_string(),
            "Cargo.toml".to_string(),
            "--cpp-view".to_string(),
        ])
        .unwrap();
        assert!(matches!(cmd, Command::Analyze { cpp_view: true, .. }));
    }

    #[test]
    fn parses_cpp_group_by_flag() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "analyze".to_string(),
            "--elf".to_string(),
            "Cargo.toml".to_string(),
            "--group-by".to_string(),
            "cpp-class".to_string(),
        ])
        .unwrap();
        assert!(matches!(cmd, Command::Analyze { group_by: CppGroupBy::CppClass, .. }));
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
            "--sarif".to_string(),
            "out.sarif".to_string(),
            "--sarif-base-uri".to_string(),
            "file:///workspace/".to_string(),
            "--sarif-min-level".to_string(),
            "info".to_string(),
            "--sarif-include-pass".to_string(),
            "true".to_string(),
            "--sarif-tool-name".to_string(),
            "fwmap-ci".to_string(),
            "--why-linked-top".to_string(),
            "7".to_string(),
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
            "--policy".to_string(),
            "Cargo.toml".to_string(),
            "--profile".to_string(),
            "release".to_string(),
            "--policy-dump-effective".to_string(),
            "--demangle=on".to_string(),
            "--map-format".to_string(),
            "lld-native".to_string(),
            "--ci-summary".to_string(),
            "--fail-on-warning".to_string(),
        ])
        .unwrap();
        match cmd {
            Command::Analyze {
                report_json,
                why_linked_top,
                sarif,
                sarif_base_uri,
                sarif_min_level,
                sarif_include_pass,
                sarif_tool_name,
                ci_summary,
                ci_format,
                ci_out,
                ci_source_summary,
                rules,
                demangle,
                map_format,
                fail_on_warning,
                max_source_diff_items,
                min_line_diff_bytes,
                hide_unknown_source,
                thresholds,
                policy,
                profile,
                policy_dump_effective,
                ..
            } => {
                assert_eq!(report_json.unwrap(), PathBuf::from("out.json"));
                assert_eq!(sarif.unwrap(), PathBuf::from("out.sarif"));
                assert_eq!(sarif_base_uri.as_deref(), Some("file:///workspace/"));
                assert_eq!(sarif_min_level, WarningLevel::Info);
                assert!(sarif_include_pass);
                assert_eq!(sarif_tool_name, "fwmap-ci");
                assert_eq!(why_linked_top, 7);
                assert!(matches!(ci_format, Some(CiFormat::Markdown)));
                assert_eq!(ci_out.unwrap(), PathBuf::from("ci.md"));
                assert_eq!(rules.unwrap(), PathBuf::from("Cargo.toml"));
                assert_eq!(policy.unwrap(), PathBuf::from("Cargo.toml"));
                assert_eq!(profile.as_deref(), Some("release"));
                assert!(policy_dump_effective);
                assert!(ci_summary);
                assert!(ci_source_summary);
                assert!(fail_on_warning);
                assert_eq!(max_source_diff_items, 7);
                assert_eq!(min_line_diff_bytes, 64);
                assert!(hide_unknown_source);
                assert!(matches!(demangle, DemangleMode::On));
                assert_eq!(map_format, MapFormatSelection::LldNative);
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
    fn parses_history_commits_command() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "history".to_string(),
            "commits".to_string(),
            "--repo".to_string(),
            ".".to_string(),
            "--limit".to_string(),
            "50".to_string(),
            "--order".to_string(),
            "ancestry".to_string(),
            "--json".to_string(),
        ])
        .unwrap();
        match cmd {
            Command::HistoryCommits { limit, order, json, .. } => {
                assert_eq!(limit, 50);
                assert_eq!(order, CommitOrder::Ancestry);
                assert!(json);
            }
            _ => panic!("expected history commits command"),
        }
    }

    #[test]
    fn parses_history_range_command() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "history".to_string(),
            "range".to_string(),
            "main...feature/foo".to_string(),
            "--include-changed-files".to_string(),
            "--html".to_string(),
            "range.html".to_string(),
        ])
        .unwrap();
        match cmd {
            Command::HistoryRange {
                spec,
                include_changed_files,
                html,
                ..
            } => {
                assert_eq!(spec, "main...feature/foo");
                assert!(include_changed_files);
                assert_eq!(html, Some(PathBuf::from("range.html")));
            }
            _ => panic!("expected history range command"),
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

    #[test]
    fn parses_explain_command() {
        let cmd = parse_args(vec![
            "fwmap".to_string(),
            "explain".to_string(),
            "--elf".to_string(),
            "Cargo.toml".to_string(),
            "--symbol".to_string(),
            "main".to_string(),
        ])
        .unwrap();
        match cmd {
            Command::Explain { symbol, object, section, .. } => {
                assert_eq!(symbol.as_deref(), Some("main"));
                assert!(object.is_none());
                assert!(section.is_none());
            }
            _ => panic!("expected explain command"),
        }
    }
}
