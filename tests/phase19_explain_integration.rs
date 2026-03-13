use fwmap::analyze::{analyze_paths, AnalyzeOptions};
use fwmap::linkage::{explain_object, explain_section, explain_symbol};
use fwmap::model::DwarfMode;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn explain_archive_keep_and_symbol_fallback_paths() {
    let dir = temp_dir("phase19-explain");
    fs::create_dir_all(&dir).unwrap();
    let elf_path = dir.join("sample.elf");
    fs::write(&elf_path, build_plain_elf32()).unwrap();

    let analysis = analyze_paths(
        &elf_path,
        Some(&PathBuf::from("tests/fixtures/sample.map")),
        Some(&PathBuf::from("tests/fixtures/sample.ld")),
        &AnalyzeOptions {
            dwarf_mode: DwarfMode::Off,
            ..AnalyzeOptions::default()
        },
    )
    .unwrap();

    let object = explain_object(&analysis, "libapp.a(startup.o)").unwrap();
    let section = explain_section(&analysis, ".text").unwrap();
    let symbol = explain_symbol(&analysis, "main").unwrap();

    assert!(object.summary.contains("archive member"));
    assert!(object.evidence.iter().any(|item| item.detail.contains("contributes")));
    assert!(section.summary.contains("placed"));
    assert!(section.evidence.iter().any(|item| item.detail.contains("Linker script")));
    assert!(symbol.summary.contains("Candidate contributing object") || symbol.summary.contains("final ELF symbol table"));

    let _ = fs::remove_file(elf_path);
    let _ = fs::remove_dir(dir);
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("fwmap-{label}-{nanos}"))
}

fn build_plain_elf32() -> Vec<u8> {
    let shstrtab = b"\0.shstrtab\0.text\0.symtab\0.strtab\0";
    let strtab = b"\0main\0";
    let mut data = vec![0u8; 0x320];
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
    write_shdr32(&mut data, shdr + 80, 11, 1, 0x6, 0x0, 0x1c0, 0x50, 0, 0, 4, 0);
    write_shdr32(&mut data, shdr + 120, 17, 2, 0, 0, 0x220, 32, 4, 1, 4, 16);
    write_shdr32(&mut data, shdr + 160, 25, 3, 0, 0, 0x240, strtab.len() as u32, 0, 0, 1, 0);

    data[0x180..0x180 + shstrtab.len()].copy_from_slice(shstrtab);
    data[0x1c0..0x210].fill(0);
    write_sym32(&mut data, 0x220, 0, 0, 0, 0, 0, 0);
    write_sym32(&mut data, 0x230, 1, 0, 16, 0x12, 0, 2);
    data[0x240..0x240 + strtab.len()].copy_from_slice(strtab);
    data
}

fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_sym32(buf: &mut [u8], offset: usize, name: u32, value: u32, size: u32, info: u8, other: u8, shndx: u16) {
    write_u32(buf, offset, name);
    write_u32(buf, offset + 4, value);
    write_u32(buf, offset + 8, size);
    buf[offset + 12] = info;
    buf[offset + 13] = other;
    write_u16(buf, offset + 14, shndx);
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
