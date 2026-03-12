# Fixture Notes

This directory contains small text fixtures used by parser and regression tests.

- `sample.map`: nominal GNU ld style output
- `broken.map`: partially broken map that should still parse
- `archive_colon.map`: archive member in `archive.a:member.o` form
- `no_memory_config.map`: map without a `Memory Configuration` block
- `decimal_sizes.map`: decimal addresses and sizes
- `tab_indented.map`: contribution line indented with a tab
- `load_address.map`: output section with `load address`
- `unparsed_block.map`: ignorable common-symbol block
- `mixed_case_regions.map`: region names in upper case
- `discarded_sections.map`: discarded sections block that should not count as contributions
- `non_ascii.map`: UTF-8 object paths and long non-ASCII names
- `sample_rules.toml`: external Phase 7 rule configuration example
- `sample.ld`: linker script subset for region and placement tests
- `sample_lld.map`: LLVM lld style map snippet used for toolchain auto-detection and normalization tests
