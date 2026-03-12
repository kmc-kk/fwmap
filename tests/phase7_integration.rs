use fwmap::analyze::{analyze_paths, AnalyzeOptions};
use fwmap::model::DemangleMode;
use fwmap::rule_config::{apply_threshold_overrides, load_rule_config};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn analyze_applies_demangle_and_external_rules() {
    let dir = temp_dir("phase7");
    fs::create_dir_all(&dir).unwrap();
    let elf_path = dir.join("cpp.elf");
    let map_path = dir.join("sample.map");
    let rules_path = PathBuf::from("tests/fixtures/sample_rules.toml");

    fs::write(&elf_path, build_cpp_elf32()).unwrap();
    fs::write(&map_path, include_str!("fixtures/sample.map")).unwrap();

    let config = load_rule_config(&rules_path).unwrap();
    let mut options = AnalyzeOptions {
        demangle: DemangleMode::On,
        custom_rules: config.rules,
        ..AnalyzeOptions::default()
    };
    apply_threshold_overrides(&mut options.thresholds, &config.thresholds);

    let result = analyze_paths(&elf_path, Some(&map_path), None, &options).unwrap();
    assert!(result
        .symbols
        .iter()
        .any(|symbol| symbol.name == "_ZN3net9g_rx_ringE" && symbol.demangled_name.as_deref().is_some()));
    assert!(result.warnings.iter().any(|warning| warning.code == "forbid-g-rx-ring"));

    let _ = fs::remove_file(elf_path);
    let _ = fs::remove_file(map_path);
    let _ = fs::remove_dir(dir);
}

#[test]
fn demangle_off_keeps_raw_symbol_name_only() {
    let dir = temp_dir("phase7-off");
    fs::create_dir_all(&dir).unwrap();
    let elf_path = dir.join("cpp.elf");
    fs::write(&elf_path, build_cpp_elf32()).unwrap();

    let options = AnalyzeOptions {
        demangle: DemangleMode::Off,
        ..AnalyzeOptions::default()
    };
    let result = analyze_paths(&elf_path, None, None, &options).unwrap();
    let symbol = result.symbols.iter().find(|symbol| symbol.name == "_ZN3net9g_rx_ringE").unwrap();
    assert!(symbol.demangled_name.is_none());

    let _ = fs::remove_file(elf_path);
    let _ = fs::remove_dir(dir);
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("fwmap-{label}-{nanos}"))
}

fn build_cpp_elf32() -> Vec<u8> {
    let strtab = b"\0_ZN3net9g_rx_ringE\0";
    let shstrtab = b"\0.shstrtab\0.text\0.symtab\0.strtab\0";
    let mut data = vec![0u8; 0x300];
    data[0..4].copy_from_slice(b"\x7fELF");
    data[4] = 1;
    data[5] = 1;
    data[6] = 1;
    write_u16(&mut data, 16, 2);
    write_u16(&mut data, 18, 0x28);
    write_u32(&mut data, 20, 1);
    write_u32(&mut data, 32, 0x80);
    write_u16(&mut data, 40, 52);
    write_u16(&mut data, 46, 40);
    write_u16(&mut data, 48, 5);
    write_u16(&mut data, 50, 1);

    let shdr = 0x80usize;
    write_shdr32(&mut data, shdr + 40, 1, 3, 0, 0, 0x180, shstrtab.len() as u32, 0, 0, 1, 0);
    write_shdr32(&mut data, shdr + 80, 11, 1, 0x6, 0x8000, 0x1b0, 4, 0, 0, 4, 0);
    write_shdr32(&mut data, shdr + 120, 17, 2, 0, 0, 0x1b4, 32, 4, 1, 4, 16);
    write_shdr32(&mut data, shdr + 160, 25, 3, 0, 0, 0x1d4, strtab.len() as u32, 0, 0, 1, 0);

    data[0x180..0x180 + shstrtab.len()].copy_from_slice(shstrtab);
    data[0x1b0..0x1b4].copy_from_slice(&[0x00, 0xbe, 0x00, 0x20]);
    write_sym32(&mut data, 0x1b4, 0, 0, 0, 0, 0, 0);
    write_sym32(&mut data, 0x1c4, 1, 0x8000, 4, 0x12, 0, 2);
    data[0x1d4..0x1d4 + strtab.len()].copy_from_slice(strtab);
    data
}

fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_shdr32(
    buf: &mut [u8],
    offset: usize,
    name: u32,
    kind: u32,
    flags: u32,
    addr: u32,
    file_offset: u32,
    size: u32,
    link: u32,
    info: u32,
    addralign: u32,
    entsize: u32,
) {
    write_u32(buf, offset, name);
    write_u32(buf, offset + 4, kind);
    write_u32(buf, offset + 8, flags);
    write_u32(buf, offset + 12, addr);
    write_u32(buf, offset + 16, file_offset);
    write_u32(buf, offset + 20, size);
    write_u32(buf, offset + 24, link);
    write_u32(buf, offset + 28, info);
    write_u32(buf, offset + 32, addralign);
    write_u32(buf, offset + 36, entsize);
}

fn write_sym32(buf: &mut [u8], offset: usize, name: u32, value: u32, size: u32, info: u8, other: u8, shndx: u16) {
    write_u32(buf, offset, name);
    write_u32(buf, offset + 4, value);
    write_u32(buf, offset + 8, size);
    buf[offset + 12] = info;
    buf[offset + 13] = other;
    write_u16(buf, offset + 14, shndx);
}
