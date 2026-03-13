# fwmap

`fwmap` is a CLI tool that analyzes firmware `ELF` and linker `map` outputs from GNU ld and LLVM lld, then emits a single-file HTML report focused on ROM/RAM usage, large symbols, object contributions, and build-to-build diffs.

## Scope

- ELF32 / ELF64 parsing
- GNU ld and LLVM lld style map parsing
- explicit `--map-format auto|gnu|lld-native` selection for native map text detection
- GNU ld linker script subset parsing (`MEMORY`, `SECTIONS`, `> REGION`, `AT`, `ALIGN`, `KEEP`)
- DWARF line-table parsing with `gimli`
- ROM/RAM summary and section breakdown
- Top symbols and top object contributions
- Optional previous-build diff
- Classified diff analysis for sections, symbols, objects, and archive members
- Memory region overview and section-to-region placement summary
- Fixed-threshold warnings
- Rule-based warning evaluation
- External TOML rule configuration
- C++ symbol demangling control
- Optional DWARF-backed source file, function, and line-range attribution
- Separate debug, build-id, and split DWARF sidecar resolution
- JSON report output
- SARIF 2.1.0 report output
- Why-linked explanation for symbols, objects, archive members, and sections
- CI summary in text / markdown / JSON formats
- warning-based exit control
- SQLite-backed history recording and trend inspection
- Graceful degradation for missing symbol tables and partially broken map files
- Toolchain auto-detection and parser-family selection
- `--verbose` and `--version` CLI support
- Offline HTML report generation

## Usage

```bash
cargo run -- analyze --elf path/to/app.elf
```

Version and verbose output:

```bash
cargo run -- --version
cargo run -- analyze --elf path/to/app.elf --verbose
```

With map information:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --toolchain auto \
  --lds linker/app.ld \
  --out report.html
```

With previous build diff:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --out report.html
```

Explain why a symbol or object was linked:

```bash
cargo run -- explain \
  --elf build/app.elf \
  --map build/app.map \
  --lds linker/app.ld \
  --symbol main
```

Default output path is `fwmap_report.html`.

History examples:

```bash
cargo run -- history record --db history.db --elf build/app.elf --map build/app.map --meta commit=abc123
cargo run -- history list --db history.db
cargo run -- history show --db history.db --build 1
cargo run -- history trend --db history.db --metric rom --last 20
cargo run -- history trend --db history.db --metric source:src/main.cpp --last 20
cargo run -- history trend --db history.db --metric function:src/main.cpp::_ZN3app4mainEv --last 20
cargo run -- history trend --db history.db --metric directory:src/app --last 20
cargo run -- history trend --db history.db --metric unknown_source --last 20
```

JSON report example:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --rules tests/fixtures/sample_rules.toml \
  --demangle=on \
  --toolchain lld \
  --dwarf=auto \
  --source-lines files \
  --report-json report.json
```

SARIF report example:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --sarif report.sarif \
  --sarif-base-uri file:///workspace/ \
  --sarif-min-level warn
```

DWARF-backed source summary example:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --dwarf=on \
  --source-lines lines \
  --source-root . \
  --path-remap build=src \
  --report-json fwmap_sources.json
```

Separate debug and debug artifact trace example:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --dwarf=on \
  --source-lines lines \
  --debug-file-dir build/debug \
  --debug-trace
```

DWARF-backed source ranking example:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --demangle=on \
  --dwarf=on \
  --source-lines all \
  --out fwmap_sources.html
```

CI-oriented example:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --ci-format markdown \
  --ci-source-summary \
  --max-source-diff-items 8 \
  --min-line-diff-bytes 64 \
  --ci-out fwmap_ci.md \
  --fail-on-warning \
  --threshold-rom 90 \
  --threshold-ram 90 \
  --threshold-region FLASH:92 \
  --threshold-symbol-growth 8192 \
  --rules tests/fixtures/sample_rules.toml
```

When previous artifacts are present, the CLI also emits a short diff summary such as:

```text
ROM: +12345 bytes
RAM: +2048 bytes
Top growth symbol: foo_bar (+4096)
Top growth object: drivers/net.o (+8192)
```

## Report Contents

- Overview: binary metadata, section count, ROM/RAM totals, warning count, optional diff totals
- Warnings: threshold violations and parser warnings
- External Rules: custom rule hits loaded from TOML
- Memory Summary: section totals with ROM/RAM classification
- Memory Regions Overview: region used/free and usage bars from linker script data
- Region Sections: sections grouped under each region
- Section Breakdown: per-section address, flags, and size
- Top Symbols: largest symbols from the ELF symbol table
- Top Object Contributions: object sizes from the map file
- Diff: summary cards plus top section/symbol/object growth and added/removed lists
- Why Linked: top diff growth items with evidence-backed link explanations
- Source Diff: top growing source files, functions, line ranges, and unknown-source delta
- Source Files: top file-level attribution with function counts
- Top Functions: symbol-linked function attribution with raw/demangled names
- Line Hotspots: compressed source line ranges with byte totals
- JSON: machine-readable report with binary, memory, warnings, diff, and region data
- SARIF: GitHub code scanning friendly warning output with rule ids, levels, locations, and stable fingerprints
- CI summary: compact text / markdown / JSON output for CI logs and PR comments

## JSON Schema

The JSON report uses a fixed top-level shape:

```json
{
  "schema_version": 1,
  "binary": { "...": "..." },
  "toolchain": { "...": "..." },
  "debug_info": { "...": "..." },
  "linker_script": { "...": "..." },
  "section_summary": [],
  "memory_summary": { "...": "..." },
  "warnings": [],
  "thresholds": { "...": "..." },
  "top_symbols": [],
  "top_object_contributions": [],
  "archive_contributions": [],
  "source_files": [],
  "functions": [],
  "line_hotspots": [],
  "line_attributions": [],
  "unknown_source": { "...": "..." },
  "regions": [],
  "diff_summary": { "...": "..." },
  "diff": { "...": "..." },
  "why_linked": { "...": "..." }
}
```

`diff_summary` and `diff` are `null` when no previous build is provided.

`top_symbols` keep both raw `name` and optional `demangled_name`, so downstream tooling can use stable raw keys while rendering readable C++ names.

## Test Fixtures

- [tests/fixtures/sample.map](tests/fixtures/sample.map)
- [tests/fixtures/broken.map](tests/fixtures/broken.map)
- [tests/fixtures/README.md](tests/fixtures/README.md)
- [tests/fixtures/sample_rules.toml](tests/fixtures/sample_rules.toml)
- [tests/fixtures/sample.ld](tests/fixtures/sample.ld)
- [tests/fixtures/sample_lld.map](tests/fixtures/sample_lld.map)

`tests/fixtures/` now contains 10+ small regression assets for map variations and parser failure modes. ELF parser tests still generate minimal synthetic ELF fixtures in test code so the repository stays lightweight.

## External Rules

Use `--rules <path>` to load a TOML rule file. If omitted, built-in rules are used.

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --rules tests/fixtures/sample_rules.toml
```

Supported custom rule kinds:

- `region_usage`
- `section_delta`
- `symbol_delta`
- `symbol_match`
- `object_match`
- `source_path_growth`
- `function_growth`
- `unknown_source_ratio`

Example:

```toml
schema_version = 1

[thresholds]
rom_usage_warn = 0.85
ram_usage_warn = 0.85
unknown_source_warn = 0.15

[[rules]]
id = "dtcm-near-full"
kind = "region_usage"
region = "DTCM"
warn_if_greater_than = 0.90
severity = "warn"
message = "DTCM usage is above 90%"

[[rules]]
id = "app_sources_growth"
kind = "source_path_growth"
pattern = "src/app/**"
threshold_bytes = 4096
severity = "warn"
message = "app sources grew by more than 4 KiB"
```

## Demangling

Use `--demangle=auto|on|off` to control C++ symbol demangling. `auto` only attempts Itanium-style names, `on` forces a demangle attempt, and `off` preserves raw symbol names.

## DWARF Source Lines

Use `--dwarf=auto|on|off` to control DWARF line-table usage and `--source-lines off|files|functions|lines|all` to choose the aggregation level.

- `auto`: use DWARF when `.debug_line` is present, otherwise fall back silently
- `on`: require DWARF and return an error when line info is missing
- `off`: skip DWARF parsing entirely

Path controls:

- `--source-root <path>` prefixes relative source paths
- `--path-remap <from=to>` remaps DWARF path prefixes and can be repeated
- `--fail-on-missing-dwarf` upgrades missing DWARF from fallback to error
- `--debug-file-dir <path>` adds a directory searched for separate debug files and split DWARF sidecars
- `--debug-trace` prints the resolution steps used to locate debug artifacts
- `--debuginfod=auto|on|off`, `--debuginfod-url`, and `--debuginfod-cache-dir` control graceful debuginfod fallback metadata

The source attribution is intentionally approximate for optimized builds because line tables reflect compiler output, not source order.
Line-0 or compiler-generated ranges are counted into `unknown_source` instead of being silently dropped, which makes partial attribution easier to diagnose.
Debug artifacts are resolved in this order: embedded debug sections, user-provided debug dirs, `.gnu_debuglink`, build-id lookup, split DWARF sidecars, then optional debuginfod fallback.
Split DWARF is supported at a basic sidecar level: `.dwo` / `.dwp` artifacts are used when they can be resolved, and unresolved or unsupported variants degrade with explicit warnings instead of aborting the whole analysis.

When DWARF and symbols are both available, `fwmap` rolls byte counts up into:

- source files
- top functions
- compressed line hotspots such as `src/main.cpp:120-134`

## Toolchains

Use `--toolchain auto|gnu|lld|iar|armcc|keil` to control map parser selection.

- `auto`: detect from map content and fall back to `gnu`
- `gnu`: force the GNU ld parser
- `lld`: force the LLVM lld parser
- `iar`, `armcc`, `keil`: reserved placeholders that currently return a clear `not implemented` error

Use `--map-format auto|gnu|lld-native` to control map text detection.

- `auto`: detect from map text headers
- `gnu`: force the GNU ld text parser
- `lld-native`: force LLVM `ld.lld` native text map parsing

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --toolchain auto
```

## CI Output

Use `--ci-format text|markdown|json` to select CI summary output. `--ci-summary` remains available as a shorthand for text output. Use `--ci-out <path>` to write the CI summary to a file.

Source-diff oriented flags:

- `--ci-source-summary` includes top growing source files, functions, and line ranges
- `--max-source-diff-items <n>` limits source diff rows
- `--min-line-diff-bytes <n>` suppresses tiny line-range noise
- `--hide-unknown-source` omits unknown-source diff rows from summaries
- repeated analysis of the same ELF within one process reuses an in-memory DWARF parse cache and reports `cache_hit` in `debug_info`

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --ci-format markdown \
  --ci-out fwmap_ci.md
```

Exit codes:

- `0`: analysis succeeded and no error-severity rule fired
- `1`: execution/input error, or `--fail-on-warning` triggered on non-error warnings
- `2`: analysis succeeded and at least one error-severity rule fired

## SARIF Output

Use `--sarif <path>` to emit SARIF 2.1.0 for GitHub code scanning and similar consumers.

- `--sarif-base-uri <uri>` maps repo-relative source paths through `originalUriBaseIds`
- `--sarif-min-level info|warn|error` filters which warnings are emitted
- `--sarif-include-pass true|false` controls pass metadata in SARIF properties
- `--sarif-tool-name <name>` overrides `tool.driver.name`

`fwmap` maps line-level findings to SARIF regions when DWARF attribution is available. File-level findings fall back to artifact-only locations, and symbol-oriented findings retain extra identity in `properties` plus a stable `partialFingerprints["fwmap/v1"]` value.

## Why Linked

Use `cargo run -- explain` to inspect why a symbol, object, archive member, or section ended up in the final image.

- `--symbol <name>` explains a symbol from the final ELF
- `--object <path|archive.a(member.o)>` explains a direct object or archive member
- `--section <name>` explains linker-script placement and `KEEP` influence
- `--why-linked-top <n>` adds top diff explanations to HTML / JSON / CI output

Current evidence sources are map contributions, archive membership, ELF symbol placement, linker-script section placement, and entry-symbol heuristics. When the exact undefined-reference chain is unavailable, `fwmap` marks the result as low or medium confidence instead of overstating certainty.

## History

Use `history record` to store one analysis result in SQLite, then inspect it with `list`, `show`, and `trend`.

```bash
cargo run -- history record \
  --db history.db \
  --elf build/app.elf \
  --map build/app.map \
  --meta commit=abc123 \
  --meta branch=main
```

Trend metrics:

- `rom`
- `ram`
- `warnings`
- `unknown_source`
- `region:<name>`
- `section:<name>`
- `source:<path>`
- `function:<path>::<raw_symbol>`
- `directory:<path>`

History details now also include DWARF availability, unknown-source ratio, top source files, and top functions for each recorded build.

The HTML report now includes lightweight client-side search and filtering for source files, functions, line hotspots, memory regions, and region sections. Long paths are shortened in tables while preserving the full path in hover tooltips, and each source row links to a ready-made history trend command block.

## Development

```bash
cargo test
```

## Current Limitations

- ELF parsing currently reads the standard symbol table (`SHT_SYMTAB`) only.
- `map` parsing targets common GNU ld / LLVM lld output and intentionally tolerates unknown lines with warnings.
- Toolchain metadata now records linker family, map format, and parser warning count in CLI / HTML / JSON / history output.
- Linker script support is currently a subset parser aimed at common GNU ld patterns.
- Object paths are sourced from the map file; when `--map` is omitted, symbol-to-object mapping is unavailable.
- Region usage relies on linker script declarations plus ELF section addresses, so unusual scripts may only be partially represented.
- JSON schema is fixed at `schema_version = 1`.
- SARIF output targets GitHub-compatible SARIF 2.1.0 fields rather than the full schema surface.
- Demangling currently prioritizes Itanium ABI names and falls back safely when conversion fails.
- History storage currently uses a local SQLite file and focuses on summary, section, region, rule-result, and source-attribution metrics.
- Toolchain auto-detection is intentionally lightweight and currently keys off GNU ld / LLVM lld map patterns only.
- `lld-native` parsing is aimed at ELF `ld.lld -Map` / `--print-map` text output and may not cover every future column variation.
- DWARF attribution uses line tables plus ELF symbol ranges; optimized builds may still collapse, duplicate, or split line ranges.
- Separate debug lookup supports user dirs, `.gnu_debuglink`, build-id paths, and basic split DWARF sidecars.
- `debuginfod` currently records provenance and degrades cleanly when lookup is unavailable; remote fetch itself is not implemented yet.

## Additional Docs

- [Toolchain support](docs/toolchains.md)

## CI Examples

GitHub Actions:

```yaml
- name: Analyze firmware size
  run: >
    cargo run -- analyze
    --elf build/app.elf
    --map build/app.map
    --prev-elf prev/app.elf
    --prev-map prev/app.map
    --rules tests/fixtures/sample_rules.toml
    --report-json fwmap.json
    --ci-format markdown
    --ci-out fwmap_ci.md
```

GitLab CI:

```yaml
fwmap:
  script:
    - cargo run -- analyze --elf build/app.elf --map build/app.map --report-json fwmap.json --ci-format json --ci-out fwmap_ci.json
  artifacts:
    paths:
      - fwmap_ci.json
      - fwmap_report.html
      - fwmap.json
```
