use std::path::{Path, PathBuf};

use crate::analyze::{analyze_paths, diff_results, evaluate_warnings};
use crate::render::{print_cli_summary, write_html_report};

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
            prev_elf,
            prev_map,
            out,
            verbose,
        } => {
            let mut current = analyze_paths(&elf, map.as_deref())?;
            let diff = if let Some(prev_elf) = prev_elf.as_deref() {
                let previous = analyze_paths(prev_elf, prev_map.as_deref())?;
                let diff = diff_results(&current, &previous);
                let mut warnings = current
                    .warnings
                    .iter()
                    .filter(|warning| warning.source != crate::model::WarningSource::Analyze)
                    .cloned()
                    .collect::<Vec<_>>();
                warnings.extend(evaluate_warnings(&current, Some(&diff)));
                current.warnings = warnings;
                Some(diff)
            } else {
                None
            };
            print_cli_summary(&current, diff.as_ref(), verbose);
            write_html_report(&out, &current, diff.as_ref())?;
            println!("Report: {}", out.display());
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
        prev_elf: Option<PathBuf>,
        prev_map: Option<PathBuf>,
        out: PathBuf,
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
    let mut prev_elf = None;
    let mut prev_map = None;
    let mut out = PathBuf::from(DEFAULT_OUT);
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
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
            "--elf" | "--map" | "--prev-elf" | "--prev-map" | "--out" => {
                let value = args.get(index + 1).ok_or_else(|| format!("missing value for {key}"))?;
                match key.as_str() {
                    "--elf" => elf = Some(PathBuf::from(value)),
                    "--map" => map = Some(PathBuf::from(value)),
                    "--prev-elf" => prev_elf = Some(PathBuf::from(value)),
                    "--prev-map" => prev_map = Some(PathBuf::from(value)),
                    "--out" => out = PathBuf::from(value),
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
    if let Some(path) = prev_map.as_deref() {
        ensure_exists(path, "previous map")?;
    }

    Ok(Command::Analyze {
        elf,
        map,
        prev_elf,
        prev_map,
        out,
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

fwmap analyze --elf <path> [--map <path>] [--prev-elf <path>] [--prev-map <path>] [--out <path>] [--verbose]

Options:
  --elf       Input ELF file (required)
  --map       GNU ld map file
  --prev-elf  Previous ELF file for diff
  --prev-map  Previous map file for diff
  --out       Output HTML path (default: fwmap_report.html)
  --verbose   Print detailed warnings to the console
  --version   Show version
  --help      Show this help"
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_args, Command};

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
    fn parses_version_flag() {
        let cmd = parse_args(vec!["fwmap".to_string(), "--version".to_string()]).unwrap();
        assert!(matches!(cmd, Command::Version));
    }
}
