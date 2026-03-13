use fwmap::analyze::{analyze_paths, AnalyzeOptions};
use fwmap::model::{DwarfMode, SourceLinesMode};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn analyze_uses_dwarf_line_table_with_path_remap() {
    let dir = temp_dir("phase11-dwarf");
    fs::create_dir_all(&dir).unwrap();
    let elf_path = dir.join("sample_dwarf.elf");
    fs::write(&elf_path, build_dwarf_elf32()).unwrap();

    let options = AnalyzeOptions {
        dwarf_mode: DwarfMode::On,
        source_lines: SourceLinesMode::Lines,
        path_remaps: vec![("src".to_string(), "/workspace/src".to_string())],
        ..AnalyzeOptions::default()
    };
    let result = analyze_paths(&elf_path, None, None, &options).unwrap();

    assert!(result.debug_info.dwarf_used);
    assert_eq!(result.debug_info.compilation_units, 1);
    assert!(result.source_files.iter().any(|item| item.path == "/workspace/src/main.c"));
    assert!(result
        .function_attributions
        .iter()
        .any(|item| item.raw_name == "main" && item.size == 8));
    assert!(result
        .line_hotspots
        .iter()
        .any(|item| item.path == "/workspace/src/main.c" && item.line_start == 10 && item.line_end == 10 && item.size == 4));
    assert!(result
        .line_attributions
        .iter()
        .any(|item| item.location.path == "/workspace/src/main.c" && item.location.line == 10 && item.size == 4));

    let _ = fs::remove_file(elf_path);
    let _ = fs::remove_dir(dir);
}

#[test]
fn dwarf_on_fails_when_debug_line_is_missing() {
    let dir = temp_dir("phase11-missing");
    fs::create_dir_all(&dir).unwrap();
    let elf_path = dir.join("sample.elf");
    fs::write(&elf_path, build_plain_elf32()).unwrap();

    let err = analyze_paths(
        &elf_path,
        None,
        None,
        &AnalyzeOptions {
            dwarf_mode: DwarfMode::On,
            source_lines: SourceLinesMode::Files,
            ..AnalyzeOptions::default()
        },
    )
    .unwrap_err();
    assert!(err.contains(".debug_line"));

    let _ = fs::remove_file(elf_path);
    let _ = fs::remove_dir(dir);
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("fwmap-{label}-{nanos}"))
}

fn build_dwarf_elf32() -> Vec<u8> {
    let shstrtab = b"\0.shstrtab\0.text\0.debug_abbrev\0.debug_info\0.debug_line\0.symtab\0.strtab\0";
    let debug_abbrev = build_debug_abbrev();
    let debug_info = build_debug_info();
    let debug_line = build_debug_line();
    let strtab = b"\0main\0";

    let mut data = vec![0u8; 0x500];
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
    write_u16(&mut data, 48, 8);
    write_u16(&mut data, 50, 1);

    let shdr = 0x80usize;
    write_shdr32(&mut data, shdr + 40, 1, 3, 0, 0, 0x340, shstrtab.len() as u32, 0, 0, 1, 0);
    write_shdr32(&mut data, shdr + 80, 11, 1, 0x6, 0x1000, 0x200, 8, 0, 0, 4, 0);
    write_shdr32(&mut data, shdr + 120, 17, 1, 0, 0, 0x208, debug_abbrev.len() as u32, 0, 0, 1, 0);
    write_shdr32(&mut data, shdr + 160, 31, 1, 0, 0, 0x220, debug_info.len() as u32, 0, 0, 1, 0);
    write_shdr32(&mut data, shdr + 200, 43, 1, 0, 0, 0x240, debug_line.len() as u32, 0, 0, 1, 0);
    write_shdr32(&mut data, shdr + 240, 55, 2, 0, 0, 0x300, 32, 7, 1, 4, 16);
    write_shdr32(&mut data, shdr + 280, 63, 3, 0, 0, 0x320, strtab.len() as u32, 0, 0, 1, 0);

    data[0x200..0x208].copy_from_slice(&[0x00, 0xbf, 0x00, 0x20, 0x00, 0xbf, 0x00, 0xbf]);
    data[0x208..0x208 + debug_abbrev.len()].copy_from_slice(&debug_abbrev);
    data[0x220..0x220 + debug_info.len()].copy_from_slice(&debug_info);
    data[0x240..0x240 + debug_line.len()].copy_from_slice(&debug_line);
    write_sym32(&mut data, 0x300, 0, 0, 0, 0, 0, 0);
    write_sym32(&mut data, 0x310, 1, 0x1000, 8, 0x12, 0, 2);
    data[0x320..0x320 + strtab.len()].copy_from_slice(strtab);
    data[0x340..0x340 + shstrtab.len()].copy_from_slice(shstrtab);
    data
}

fn build_plain_elf32() -> Vec<u8> {
    let shstrtab = b"\0.shstrtab\0.text\0";
    let mut data = vec![0u8; 0x260];
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
    write_u16(&mut data, 48, 3);
    write_u16(&mut data, 50, 1);

    let shdr = 0x80usize;
    write_shdr32(&mut data, shdr + 40, 1, 3, 0, 0, 0x160, shstrtab.len() as u32, 0, 0, 1, 0);
    write_shdr32(&mut data, shdr + 80, 11, 1, 0x6, 0x1000, 0x180, 8, 0, 0, 4, 0);
    data[0x160..0x160 + shstrtab.len()].copy_from_slice(shstrtab);
    data[0x180..0x188].copy_from_slice(&[0x00, 0xbf, 0x00, 0x20, 0x00, 0xbf, 0x00, 0xbf]);
    data
}

fn build_debug_abbrev() -> Vec<u8> {
    vec![
        0x01, // abbrev code
        0x11, // DW_TAG_compile_unit
        0x00, // no children
        0x03, 0x08, // DW_AT_name, DW_FORM_string
        0x1b, 0x08, // DW_AT_comp_dir, DW_FORM_string
        0x10, 0x17, // DW_AT_stmt_list, DW_FORM_sec_offset
        0x00, 0x00, // end attrs
        0x00, // end abbrev table
    ]
}

fn build_debug_info() -> Vec<u8> {
    let mut body = Vec::new();
    body.push(0x01); // abbrev code
    body.extend_from_slice(b"main.c\0");
    body.extend_from_slice(b"src\0");
    body.extend_from_slice(&0u32.to_le_bytes()); // stmt_list offset into .debug_line

    let mut info = Vec::new();
    info.extend_from_slice(&((2 + 4 + 1 + body.len()) as u32).to_le_bytes());
    info.extend_from_slice(&4u16.to_le_bytes());
    info.extend_from_slice(&0u32.to_le_bytes());
    info.push(4);
    info.extend_from_slice(&body);
    info
}

fn build_debug_line() -> Vec<u8> {
    let mut header = vec![
        1,     // min instruction length
        1,     // max ops per instruction
        1,     // default is_stmt
        0xfb,  // line_base = -5
        14,    // line_range
        13,    // opcode_base
    ];
    header.extend_from_slice(&[0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1]);
    header.extend_from_slice(b"src\0");
    header.push(0);
    header.extend_from_slice(b"main.c\0");
    header.extend_from_slice(&[1, 0, 0]);
    header.push(0);

    let mut program = Vec::new();
    program.extend_from_slice(&[0, 5, 2]);
    program.extend_from_slice(&0x1000u32.to_le_bytes());
    program.push(1); // copy line 1
    program.extend_from_slice(&[2, 4]); // advance_pc 4
    program.extend_from_slice(&[3, 9]); // advance_line +9 => line 10
    program.push(1); // copy line 10
    program.extend_from_slice(&[2, 4]); // advance_pc 4
    program.extend_from_slice(&[0, 1, 1]); // end_sequence

    let mut line = Vec::new();
    line.extend_from_slice(&0u32.to_le_bytes());
    line.extend_from_slice(&4u16.to_le_bytes());
    line.extend_from_slice(&(header.len() as u32).to_le_bytes());
    line.extend_from_slice(&header);
    line.extend_from_slice(&program);
    let unit_length = (line.len() - 4) as u32;
    line[0..4].copy_from_slice(&unit_length.to_le_bytes());
    line
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
