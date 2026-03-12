# fwmap

`fwmap` is a Rust CLI prototype that analyzes firmware `ELF` and GNU ld `map` outputs, then emits a single-file HTML report focused on ROM/RAM usage, large symbols, object contributions, and build-to-build diffs.

## Scope

- ELF32 / ELF64 parsing
- GNU ld style map parsing
- ROM/RAM summary and section breakdown
- Top symbols and top object contributions
- Optional previous-build diff
- Fixed-threshold warnings
- Offline HTML report generation

## Usage

```bash
cargo run -- analyze --elf path/to/app.elf
```

With map information:

```bash
cargo run -- analyze \
  --elf build/app.elf \
  --map build/app.map \
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

## Report Contents

- Overview: binary metadata, section count, ROM/RAM totals, warning count, optional diff totals
- Warnings: threshold violations and parser warnings
- Memory Summary: section totals with ROM/RAM classification
- Section Breakdown: per-section address, flags, and size
- Top Symbols: largest symbols from the ELF symbol table
- Top Object Contributions: object sizes from the map file
- Diff: section/symbol/object deltas plus added/removed symbols

## Test Fixtures

- [tests/fixtures/sample.map](/e:/work/git/fwmap/tests/fixtures/sample.map)
- [tests/fixtures/broken.map](/e:/work/git/fwmap/tests/fixtures/broken.map)

ELF parser tests generate a minimal synthetic ELF fixture in the test body so the repository stays text-only.

## Development

```bash
cargo test
```

## Current Limitations

- ELF parsing currently reads the standard symbol table (`SHT_SYMTAB`) only.
- `map` parsing targets common GNU ld output and intentionally tolerates unknown lines with warnings.
- Object paths are sourced from the map file; when `--map` is omitted, symbol-to-object mapping is unavailable.
- ROM/RAM estimation is heuristic and does not yet interpret linker scripts or load addresses exactly.
- Demangling is not implemented.

## Planned Extensions

- Linker script awareness and region visualization
- JSON output for CI
- Richer archive/member aggregation
- Better demangling and C++ symbol analysis
