use std::path::{Path, PathBuf};

use crate::analyze::{analyze_paths, evaluate_warnings, AnalyzeOptions};
use crate::diff::{diff_results, top_increases};
use crate::demangle::display_name;
use crate::model::{DemangleMode, ThresholdConfig};
use crate::rule_config::{apply_threshold_overrides, load_rule_config};
use crate::render::{print_ci_summary, print_cli_summary, write_html_report, write_json_report};

const DEFAULT_OUT: &str = "fwmap_report.html";
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let parsed = parse_args(args.into_iter().collect())?;
    match parsed {
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::Version => {
            println!("fwmap {VERSION}");
            Ok(())
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
            fail_on_warning,
            thresholds,
            rules,
            demangle,
            verbose,
        } => {
            let mut options = AnalyzeOptions {
                thresholds,
                demangle,
                custom_rules: Vec::new(),
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
            if ci_summary {
                print_ci_summary(&current, diff.as_ref());
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
            write_html_report(&out, &current, diff.as_ref())?;
            if let Some(path) = report_json.as_deref() {
                write_json_report(path, &current, diff.as_ref(), &options.thresholds)?;
                if !ci_summary {
                    println!("JSON: {}", path.display());
                }
            }
            if !ci_summary {
                println!("Report: {}", out.display());
            }
            if fail_on_warning && !current.warnings.is_empty() {
                return Err(format!("warning threshold exceeded: {} warning(s) triggered", current.warnings.len()));
            }
            Ok(())
        }
    }
}

#[derive(Debug)]
enum Command {
    Help,
    Version,
    Analyze {
        elf: PathBuf,
        map: Option<PathBuf>,
        lds: Option<PathBuf>,
        prev_elf: Option<PathBuf>,
        prev_map: Option<PathBuf>,
        out: PathBuf,
        report_json: Option<PathBuf>,
        ci_summary: bool,
        fail_on_warning: bool,
        thresholds: ThresholdConfig,
        rules: Option<PathBuf>,
        demangle: DemangleMode,
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
    let mut fail_on_warning = false;
    let mut thresholds = ThresholdConfig::default();
    let mut rules = None;
    let mut demangle = DemangleMode::Auto;
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
            "--fail-on-warning" => {
                fail_on_warning = true;
                index += 1;
                continue;
            }
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
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
            "--elf" | "--map" | "--lds" | "--prev-elf" | "--prev-map" | "--out" | "--report-json" | "--rules" => {
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
        fail_on_warning,
        thresholds,
        rules,
        demangle,
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

fwmap analyze --elf <path> [--map <path>] [--lds <path>] [--prev-elf <path>] [--prev-map <path>] [--out <path>] [--report-json <path>] [--rules <path>] [--demangle=auto|on|off] [--verbose]

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
  --ci-summary Print compact CI-friendly summary
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

fn parse_region_threshold(value: &str) -> Result<(String, f64), String> {
    let (name, percent) = value
        .split_once(':')
        .ok_or_else(|| format!("invalid region threshold '{value}', expected <name:percent>"))?;
    Ok((name.to_string(), parse_percent(percent, "--threshold-region")?))
}

#[cfg(test)]
mod tests {
    use super::{parse_args, Command};
    use crate::model::DemangleMode;
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
                rules,
                demangle,
                fail_on_warning,
                thresholds,
                ..
            } => {
                assert_eq!(report_json.unwrap(), PathBuf::from("out.json"));
                assert_eq!(rules.unwrap(), PathBuf::from("Cargo.toml"));
                assert!(ci_summary);
                assert!(fail_on_warning);
                assert!(matches!(demangle, DemangleMode::On));
                assert_eq!(thresholds.rom_percent, 90.0);
                assert_eq!(thresholds.region_percent.get("FLASH"), Some(&92.0));
                assert_eq!(thresholds.symbol_growth_bytes, 8192);
            }
            _ => panic!("expected analyze command"),
        }
    }
}
