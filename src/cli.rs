use std::path::{Path, PathBuf};

use crate::analyze::{analyze_paths, diff_results, evaluate_warnings};
use crate::render::{print_cli_summary, write_html_report};

const DEFAULT_OUT: &str = "fwmap_report.html";

pub fn run(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let parsed = parse_args(args.into_iter().collect())?;
    match parsed {
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::Analyze {
            elf,
            map,
            prev_elf,
            prev_map,
            out,
        } => {
            let mut current = analyze_paths(&elf, map.as_deref())?;
            let diff = if let Some(prev_elf) = prev_elf.as_deref() {
                let previous = analyze_paths(prev_elf, prev_map.as_deref())?;
                let diff = diff_results(&current, &previous);
                current.warnings = evaluate_warnings(&current, Some(&diff));
                Some(diff)
            } else {
                None
            };
            print_cli_summary(&current, diff.as_ref());
            write_html_report(&out, &current, diff.as_ref())?;
            println!("Report: {}", out.display());
            Ok(())
        }
    }
}

#[derive(Debug)]
enum Command {
    Help,
    Analyze {
        elf: PathBuf,
        map: Option<PathBuf>,
        prev_elf: Option<PathBuf>,
        prev_map: Option<PathBuf>,
        out: PathBuf,
    },
}

fn parse_args(args: Vec<String>) -> Result<Command, String> {
    if args.len() <= 1 || matches!(args.get(1).map(String::as_str), Some("--help" | "-h")) {
        return Ok(Command::Help);
    }
    if args[1] != "analyze" {
        return Err(format!("unknown command '{}'\n\n{}", args[1], help_text()));
    }

    let mut elf = None;
    let mut map = None;
    let mut prev_elf = None;
    let mut prev_map = None;
    let mut out = PathBuf::from(DEFAULT_OUT);
    let mut index = 2usize;
    while index < args.len() {
        let key = &args[index];
        let value = args.get(index + 1).ok_or_else(|| format!("missing value for {key}"))?;
        match key.as_str() {
            "--elf" => elf = Some(PathBuf::from(value)),
            "--map" => map = Some(PathBuf::from(value)),
            "--prev-elf" => prev_elf = Some(PathBuf::from(value)),
            "--prev-map" => prev_map = Some(PathBuf::from(value)),
            "--out" => out = PathBuf::from(value),
            "--help" | "-h" => return Ok(Command::Help),
            _ => return Err(format!("unknown option '{key}'")),
        }
        index += 2;
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
    })
}

fn ensure_exists(path: &Path, label: &str) -> Result<(), String> {
    if path.exists() {
        Ok(())
    } else {
        Err(format!("{label} file does not exist: {}", path.display()))
    }
}

fn print_help() {
    println!("{}", help_text());
}

fn help_text() -> String {
    "fwmap analyze --elf <path> [--map <path>] [--prev-elf <path>] [--prev-map <path>] [--out <path>]

Options:
  --elf       Input ELF file (required)
  --map       GNU ld map file
  --prev-elf  Previous ELF file for diff
  --prev-map  Previous map file for diff
  --out       Output HTML path (default: fwmap_report.html)
  --help      Show this help"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::parse_args;

    #[test]
    fn help_without_args() {
        let cmd = parse_args(vec!["fwmap".to_string()]).unwrap();
        assert!(matches!(cmd, super::Command::Help));
    }
}
