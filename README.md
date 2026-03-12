# fwmap

`fwmap` is a Rust CLI prototype that analyzes firmware `ELF` and GNU ld `map` outputs, then emits a single-file HTML report focused on ROM/RAM usage, large symbols, object contributions, and build-to-build diffs.

## Scope

- ELF32 / ELF64 parsing
- GNU ld style map parsing
- GNU ld linker script subset parsing (`MEMORY`, `SECTIONS`, `> REGION`, `AT`, `ALIGN`, `KEEP`)
- ROM/RAM summary and section breakdown
- Top symbols and top object contributions
- Optional previous-build diff
- Classified diff analysis for sections, symbols, objects, and archive members
- Memory region overview and section-to-region placement summary
- Fixed-threshold warnings
- JSON report output
- CI summary and warning-based exit control
- Graceful degradation for missing symbol tables and partially broken map files
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

Default output path is `fwmap_report.html`.

JSON report example:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --report-json report.json
```

CI-oriented example:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
  --prev-elf prev/app.elf \
  --prev-map prev/app.map \
  --ci-summary \
  --fail-on-warning \
  --threshold-rom 90 \
  --threshold-ram 90 \
  --threshold-region FLASH:92 \
  --threshold-symbol-growth 8192
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
- Memory Summary: section totals with ROM/RAM classification
- Memory Regions Overview: region used/free and usage bars from linker script data
- Region Sections: sections grouped under each region
- Section Breakdown: per-section address, flags, and size
- Top Symbols: largest symbols from the ELF symbol table
- Top Object Contributions: object sizes from the map file
- Diff: summary cards plus top section/symbol/object growth and added/removed lists
- JSON: machine-readable report with binary, memory, warnings, diff, and region data

## JSON Schema

The JSON report uses a fixed top-level shape:

```json
{
  "schema_version": 1,
  "binary": { "...": "..." },
  "linker_script": { "...": "..." },
  "section_summary": [],
  "memory_summary": { "...": "..." },
  "warnings": [],
  "thresholds": { "...": "..." },
  "top_symbols": [],
  "top_object_contributions": [],
  "archive_contributions": [],
  "regions": [],
  "diff_summary": { "...": "..." },
  "diff": { "...": "..." }
}
```

`diff_summary` and `diff` are `null` when no previous build is provided.

## Test Fixtures

- [tests/fixtures/sample.map](/e:/work/git/fwmap/tests/fixtures/sample.map)
- [tests/fixtures/broken.map](/e:/work/git/fwmap/tests/fixtures/broken.map)
- [tests/fixtures/README.md](/e:/work/git/fwmap/tests/fixtures/README.md)
- [tests/fixtures/sample.ld](/e:/work/git/fwmap/tests/fixtures/sample.ld)

`tests/fixtures/` now contains 10+ small regression assets for map variations and parser failure modes. ELF parser tests still generate minimal synthetic ELF fixtures in test code so the repository stays lightweight.

## Development

```bash
cargo test
```

## Current Limitations

- ELF parsing currently reads the standard symbol table (`SHT_SYMTAB`) only.
- `map` parsing targets common GNU ld output and intentionally tolerates unknown lines with warnings.
- Warning items now retain their source and related entity so skipped input can be explained in reports and verbose CLI output.
- Linker script support is currently a subset parser aimed at common GNU ld patterns.
- Object paths are sourced from the map file; when `--map` is omitted, symbol-to-object mapping is unavailable.
- Region usage relies on linker script declarations plus ELF section addresses, so unusual scripts may only be partially represented.
- JSON schema is fixed at `schema_version = 1`.
- Demangling is not implemented.

## CLI Compatibility

- Existing `fwmap analyze --elf ...` usage remains valid.
- `--verbose` and `--version` were added without changing existing flags.
- Phase 3 only extends diff output; existing flags and required arguments are unchanged.
- Phase 4 adds optional `--lds` without changing existing required arguments.
- Phase 5 adds optional reporting and threshold flags without changing existing required arguments.

## Planned Extensions

- Region-aware placement analysis
- Better demangling and C++ symbol analysis

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
    --report-json fwmap.json
    --ci-summary
    --fail-on-warning
```

GitLab CI:

```yaml
fwmap:
  script:
    - cargo run -- analyze --elf build/app.elf --map build/app.map --report-json fwmap.json --ci-summary
  artifacts:
    paths:
      - fwmap_report.html
      - fwmap.json
```
