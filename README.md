# fwmap

`fwmap` is a CLI tool that analyzes firmware `ELF` and linker `map` outputs from GNU ld and LLVM lld, then emits a single-file HTML report focused on ROM/RAM usage, large symbols, object contributions, and build-to-build diffs.

## Scope

- Analyze firmware `ELF` plus GNU ld / LLVM lld `map` files and emit a standalone HTML report
- Summarize ROM/RAM usage, section layout, symbols, objects, archives, and build-to-build diffs
- Attribute bytes back to source files, functions, and line ranges with optional DWARF support
- Explain why symbols, objects, archive members, and sections were linked
- Export machine-readable JSON, SARIF, and CI-oriented text / markdown / JSON summaries
- Track history in SQLite with Git-aware timeline, range diff, regression, and trend queries
- Support C++ demangling / grouping and Rust Cargo ingestion / Rust View aggregation
- Degrade cleanly when map files, symbol tables, or debug artifacts are partial or missing

## Quick Start

Build the binary:

```bash
cargo build
```

Build an optimized release binary:

```bash
cargo build --release
```

Run the CLI directly from source:

```bash
cargo run -- analyze --elf path/to/app.elf
```

Or invoke the built binary:

```bash
./target/debug/fwmap analyze --elf path/to/app.elf
```

Run the full test suite:

```bash
cargo test
```

## Desktop App

A first Tauri desktop shell now lives under `apps/fwmap-desktop/`. It keeps the existing Rust core as the analysis source of truth and adds a local GUI for file picking, job status, recent runs, run detail, and desktop settings.

Install desktop dependencies:

```bash
cd apps/fwmap-desktop
npm install
```

Run the frontend + Tauri shell in development:

```bash
cd apps/fwmap-desktop
npm run tauri dev
```

Check only the Tauri backend crate:

```bash
cargo check --manifest-path apps/fwmap-desktop/src-tauri/Cargo.toml
```

Build the desktop frontend bundle:

```bash
cd apps/fwmap-desktop
npm run build
```

The desktop app now covers the full local workflow:

- start analysis jobs, track progress, and review recent runs
- inspect commit timelines, compare recorded runs, and query git-aware history
- monitor ROM/RAM and warning trends in a visual dashboard
- manage reusable projects, policy files, and export destinations
- drill into regions, files, functions, symbols, crates, and dependencies from the Inspector

Desktop capabilities currently available:

- Start analysis from local ELF / map / rule / Git repo paths
- Track one-shot analysis jobs with Tauri events
- Persist desktop settings and recent runs in a local SQLite app database
- Record actual analysis history into the existing fwmap history database
- Browse recent runs and a compact run detail view
- Load commit timelines with branch / profile / target filters
- Compare two recorded runs with section / object / source / symbol / Rust delta lists
- Query git-aware range diffs and regression-origin summaries from the desktop UI
- Visualize recent ROM/RAM history, warning pressure, region usage, and top growth contributors in the dashboard
- Manage workspace-style projects with default paths, policy files, and export destinations
- Load, validate, save, and reuse desktop policy documents from the GUI
- Export dashboard/run/diff/history/regression snapshots as HTML, print-friendly HTML, or JSON

Current limitations:

- Job cancellation is a placeholder and does not interrupt the analysis thread
- Desktop navigation uses lightweight in-app state rather than a full router
- Deep charts, rich editors, and dense drill-down visualization are still future work
- The desktop app reuses existing fwmap core/history logic instead of replacing the CLI

## Usage

Basic analysis:

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

Rust / Cargo artifact discovery:

```bash
cargo metadata --format-version=1 > build/cargo-metadata.json
cargo build --release --message-format=json > build/cargo-build.jsonl

cargo run -- analyze \
  --cargo-build-json build/cargo-build.jsonl \
  --cargo-metadata build/cargo-metadata.json \
  --cargo-package fwmap \
  --cargo-target-name fwmap \
  --cargo-target-kind bin \
  --cargo-target-triple x86_64-unknown-linux-gnu \
  --resolve-rust-artifact strict \
  --map target/release/fwmap.map \
  --report-json report.json
```

When multiple Rust artifacts are present, narrow selection with `--cargo-package`, `--cargo-target-name`, or `--cargo-target-kind`. If you already know the artifact path, `--elf` still wins and Cargo inputs only enrich `rust_context`.

Rust View:

```bash
cargo run -- analyze \
  --elf target/release/fwmap \
  --map target/release/fwmap.map \
  --cargo-metadata build/cargo-metadata.json \
  --cargo-build-json build/cargo-build.jsonl \
  --cargo-package fwmap \
  --cargo-target-name fwmap \
  --view rust \
  --report-json report.json \
  --out report.html
```

`--view rust` keeps the generic ELF workflow intact and adds Rust-oriented summaries for packages, targets, crates, dependency crates, grouped generic/closure/async families, and Rust symbols. If Rust metadata is missing, the CLI degrades gracefully instead of failing.

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
cargo run -- analyze --elf build/app.elf --map build/app.map --save-history
cargo run -- analyze --elf build/app.elf --map build/app.map --save-history --history-db history.db --git-repo .
cargo run -- analyze --elf build/app.elf --map build/app.map --save-history --no-git
cargo run -- history record --db history.db --elf build/app.elf --map build/app.map --git-repo .
cargo run -- history list --db history.db --limit 20
cargo run -- history list --db history.db --limit 20 --json
cargo run -- history show --db history.db --build 1 --view rust
cargo run -- history trend --db history.db --metric rom --last 20
cargo run -- history trend --db history.db --metric source:src/main.cpp --last 20
cargo run -- history trend --db history.db --metric function:src/main.cpp::_ZN3app4mainEv --last 20
cargo run -- history trend --db history.db --metric object:build/main.o --last 20
cargo run -- history trend --db history.db --metric archive-member:libapp.a(startup.o) --last 20
cargo run -- history trend --db history.db --metric directory:src/app --last 20
cargo run -- history trend --db history.db --metric unknown_source --last 20
cargo run -- history trend --db history.db --metric rust-package:fwmap --last 20
cargo run -- history trend --db history.db --metric rust-dependency:tokio --last 20
cargo run -- history trend --db history.db --metric rust-family:fwmap::worker::poll --last 20
cargo run -- history commits --repo . --limit 50 --order ancestry
cargo run -- history commits --repo . --branch main --json
cargo run -- history range main~20..main --repo . --include-changed-files --view rust
cargo run -- history range main...feature/foo --repo . --json
cargo run -- history regression --metric rom_total main~50..main --threshold +8192 --repo .
cargo run -- history regression --metric rust-dependency:tokio.size main~50..main --threshold +16384 --repo .
cargo run -- history regression --rule ram-budget-exceeded main~50..main --include-evidence --json
cargo run -- history regression --entity source:src/net/proto.cpp v1.2.0..HEAD --include-changed-files --html regression.html
```

When the current working tree is inside a Git repository, history records and report output include Git metadata such as commit hash, branch, `git describe`, subject, and dirty state. Use `--git-repo <path>` to probe a specific repository or `--no-git` to disable Git collection explicitly.
`history commits` shows analyzed commits aligned to Git history order, while `history range` summarizes an `A..B` or `A...B` slice with cumulative ROM/RAM deltas, worst commit, missing-analysis count, and optional changed-files intersection.
`history regression` estimates the first analyzed commit where a metric threshold was crossed, a rule first became active, or an entity first appeared. The report includes `last_good`, `first_observed_bad`, `first_bad_candidate`, confidence, reasoning, and optional evidence such as transition rows and changed files.

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

C++ aggregate summary example:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --demangle=on \
  --cpp-view \
  --report-json fwmap_cpp.json
```

Policy-as-code example:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf build/app-prev.elf \
  --prev-map build/app-prev.map \
  --policy tests/fixtures/sample_policy_v2.toml \
  --profile release \
  --policy-dump-effective \
  --report-json fwmap_policy.json \
  --sarif fwmap_policy.sarif
```

C++ diff grouping example:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --demangle=on \
  --cpp-view \
  --group-by cpp-class
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
- Object Details: object-level why-linked summaries, confidence, and trend commands
- Archive Details: archive/member totals, whole-archive signals, and trend commands
- Diff: summary cards plus top section/symbol/object growth and added/removed lists
- Why Linked: top diff growth items with evidence-backed link explanations
- Source Diff: top growing source files, functions, line ranges, and unknown-source delta
- Source Files: top file-level attribution with function counts
- Top Functions: symbol-linked function attribution with raw/demangled names
- Line Hotspots: compressed source line ranges with byte totals
- C++ view: classified symbols plus top template families, classes, method families, lambda groups, and runtime overhead buckets
- C++ diff: template-family, class, runtime-overhead, and lambda-group growth with symbol drill-down and why-linked summaries
- Rust view: top packages, targets, crates, dependency crates, source files, grouped generic/closure/async families, and largest Rust symbols
- Rust diff: package, target, crate, dependency, family, and symbol deltas
- JSON: machine-readable report with binary, memory, warnings, diff, and region data
- SARIF: GitHub code scanning friendly warning output with rule ids, levels, locations, and stable fingerprints
- CI summary: compact text / markdown / JSON output for CI logs and PR comments

## JSON Schema

The JSON report uses a fixed top-level shape:

```json
{
  "schema_version": 1,
  "binary": { "...": "..." },
  "git": { "...": "..." },
  "rust_context": { "...": "..." },
  "rust_view": { "...": "..." },
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
  "rust_diff": { "...": "..." },
  "regions": [],
  "diff_summary": { "...": "..." },
  "diff": { "...": "..." },
  "why_linked": { "...": "..." }
}
```

`diff_summary` and `diff` are `null` when no previous build is provided.

`top_symbols` keep both raw `name` and optional `demangled_name`, so downstream tooling can use stable raw keys while rendering readable C++ names.
`rust_view` is optional and appears when Rust-attributed symbols are detected. It includes `packages`, `targets`, `crates`, `dependency_crates`, `source_files`, `grouped_families`, and `symbols`. `rust_diff` mirrors those aggregate layers for build-to-build comparisons.

## Test Fixtures

- [tests/fixtures/sample.map](tests/fixtures/sample.map)
- [tests/fixtures/broken.map](tests/fixtures/broken.map)
- [tests/fixtures/README.md](tests/fixtures/README.md)
- [tests/fixtures/sample_rules.toml](tests/fixtures/sample_rules.toml)
- [tests/fixtures/sample.ld](tests/fixtures/sample.ld)
- [tests/fixtures/sample_lld.map](tests/fixtures/sample_lld.map)
- [tests/fixtures/sample_policy_v2.toml](tests/fixtures/sample_policy_v2.toml)

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

## Policy As Code

Use `--policy <path>` to load a TOML policy file with `version = 2`. Policies add profile-specific budgets, owner resolution, and time-bounded waivers on top of the existing rule engine.

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf build/app-prev.elf \
  --prev-map build/app-prev.map \
  --policy tests/fixtures/sample_policy_v2.toml \
  --profile release
```

Supported budget scopes:

- `regions`
- `paths`
- `libraries`
- `cpp_classes`
- `cpp_template_families`

Supported policy extras:

- owner mapping by `paths`, `objects`, `libraries`, `cpp_classes`, `cpp_template_families`
- active waivers with required `reason`
- expired waiver reporting
- `--policy-dump-effective` to print the selected profile summary

Example:

```toml
version = 2
default_profile = "release"

[profiles.release.budgets.regions.FLASH]
max_bytes = 524288
warn_bytes = 500000

[profiles.release.budgets.paths."src/net/**"]
max_delta_bytes = 4096

[[owners]]
owner = "network-team"
[owners.match]
paths = ["src/net/**"]

[[waivers]]
rule = "budget.path.delta"
expires = "2026-12-31"
reason = "legacy migration in progress"
[waivers.match]
paths = ["src/legacy/**"]
```

Policy results are included in HTML, JSON, and SARIF output. JSON keeps the structured `policy` block, while SARIF carries owner and policy-profile metadata in result properties when a policy violation is emitted.

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

Current evidence sources are map contributions, GNU cross-reference tables, archive-pull tables, archive membership, ELF relocations, linker-script section placement, whole-archive heuristics, and entry-symbol heuristics. When the exact undefined-reference chain is unavailable, `fwmap` marks the result as low or medium confidence instead of overstating certainty.

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
- `object:<path>`
- `archive-member:<archive(member)>`
- `directory:<path>`

History details now also include DWARF availability, unknown-source ratio, top source files, top functions, and stored why-linked summaries for top objects in each recorded build.
When Rust View data is present, history also persists summarized Rust package / target / crate / dependency / source / family tables so `history show --view rust`, `history trend`, and regression detection can inspect Rust-oriented growth without recomputing symbol groups.

The HTML report now includes lightweight client-side search and filtering for source files, functions, line hotspots, memory regions, region sections, objects, and archives. Long paths are shortened in tables while preserving the full path in hover tooltips, and each source/object/archive row links to a ready-made history trend command block.

## Development

Prerequisites:

- Rust toolchain with `cargo`
- A sample `ELF` and optional linker `map` file if you want to exercise the CLI manually

Build commands:

```bash
cargo build
cargo build --release
```

The debug binary is written to `target/debug/fwmap` and the release binary to `target/release/fwmap`.

Common local workflows:

```bash
# basic CLI smoke check
cargo run -- --version

# analyze a local binary
cargo run -- analyze --elf build/app.elf --map build/app.map --out fwmap_report.html

# generate machine-readable output while iterating
cargo run -- analyze --elf build/app.elf --map build/app.map --report-json fwmap_report.json

# try the Rust-oriented summary
cargo run -- analyze --elf target/release/fwmap --map target/release/fwmap.map --view rust
```

Test commands:

```bash
cargo test
cargo test cli::tests
cargo test core::history::tests
cargo test report::render::tests
```

When working on parser, history, or report changes, it is usually worth running the narrower test group first and then finishing with a full `cargo test`.

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
- Rust history persistence stores normalized package / target / profile / triple context when Cargo inputs are present.
- Rust family grouping is heuristic and deterministic rather than perfect. Generic families collapse angle-bracket payloads, closures key off `{{closure}}`, async groups key off async/future/poll patterns, and trait groups key off `<T as Trait>` forms.
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
    cargo metadata --format-version=1 > build/cargo-metadata.json &&
    cargo build --release --message-format=json > build/cargo-build.jsonl &&
    cargo run -- analyze
    --cargo-build-json build/cargo-build.jsonl
    --cargo-metadata build/cargo-metadata.json
    --cargo-package fwmap
    --cargo-target-name fwmap
    --cargo-target-kind bin
    --resolve-rust-artifact strict
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
    - cargo metadata --format-version=1 > build/cargo-metadata.json
    - cargo build --release --message-format=json > build/cargo-build.jsonl
    - cargo run -- analyze --cargo-build-json build/cargo-build.jsonl --cargo-metadata build/cargo-metadata.json --cargo-target-name app --cargo-target-kind bin --resolve-rust-artifact strict --map build/app.map --report-json fwmap.json --ci-format json --ci-out fwmap_ci.json
  artifacts:
    paths:
      - fwmap_ci.json
      - fwmap_report.html
      - fwmap.json
```
