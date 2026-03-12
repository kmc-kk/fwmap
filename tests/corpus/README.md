# Corpus Layout

Use this directory for larger end-to-end samples that exercise complete toolchain families.

- `gnu/`: GNU ld based ELF/map pairs
- `lld/`: LLVM lld based ELF/map pairs
- `malformed/`: intentionally broken inputs used for degraded-mode and error-path tests
- `cpp/`: C++-heavy samples that stress demangling and long symbol names

Small single-purpose parser regressions should stay in `tests/fixtures/`.
