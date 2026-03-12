# Toolchain Support

`fwmap` accepts `--toolchain auto|gnu|lld|iar|armcc|keil`.

## Supported Families

- `auto`: detect from the map file and fall back to `gnu` when detection is inconclusive
- `gnu`: force the GNU ld style parser
- `lld`: force the LLVM lld style parser

## Placeholder Families

- `iar`
- `armcc`
- `keil`

These names are recognized so future parser families can reuse the same CLI surface. The current behavior is a clear `not implemented` error before analysis continues.

## Detection Rules

`auto` currently uses lightweight text detection:

- `lld`: a map header that contains `VMA`, `LMA`, `Out`, and `In`
- `gnu`: `Memory Configuration` or `Linker script and memory map`
- fallback: `gnu`

## Adding a New Parser Family

1. Extend `ToolchainSelection` and, if supported, `ToolchainKind`.
2. Add detection logic in [src/ingest/map/mod.rs](/e:/work/git/fwmap/src/ingest/map/mod.rs).
3. Add a parser branch that normalizes into `MapIngestResult`.
4. Add a fixture under `tests/fixtures/` or `tests/corpus/<family>/`.
5. Add unit tests for detection and parsing plus one integration test through `cli::run` or `analyze_paths`.

## Corpus Layout

Future sample intake should follow:

```text
tests/
  corpus/
    gnu/
    lld/
    malformed/
    cpp/
```

Use the corpus directories for larger toolchain-specific samples and keep `tests/fixtures/` for small focused regression snippets.
