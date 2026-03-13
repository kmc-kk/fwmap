use fwmap::analyze::{analyze_paths, AnalyzeOptions};
use fwmap::model::{DebugArtifactKind, DebugArtifactSource, DwarfMode, SourceLinesMode};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn analyze_uses_gnu_debuglink_sidecar() {
    let dir = temp_dir("phase17-debuglink");
    fs::create_dir_all(&dir).unwrap();
    let elf_path = dir.join("sample.elf");
    let sidecar_path = dir.join("sample.debug");
    fs::write(&elf_path, build_plain_elf32_with_debuglink("sample.debug")).unwrap();
    fs::write(&sidecar_path, build_dwarf_elf32()).unwrap();

    let options = AnalyzeOptions {
        dwarf_mode: DwarfMode::On,
        source_lines: SourceLinesMode::Lines,
        debug_file_dirs: vec![dir.clone()],
        ..AnalyzeOptions::default()
    };
    let result = analyze_paths(&elf_path, None, None, &options).unwrap();

    assert!(result.debug_info.dwarf_used);
    assert_eq!(result.debug_artifact.kind, DebugArtifactKind::SeparateDebug);
    assert_eq!(result.debug_artifact.source, DebugArtifactSource::UserDir);
    assert!(result.source_files.iter().any(|item| item.path.ends_with("src/main.c")));
    let _ = fs::remove_file(elf_path);
    let _ = fs::remove_file(sidecar_path);
    let _ = fs::remove_dir(dir);
}

#[test]
fn analyze_uses_build_id_sidecar() {
    let dir = temp_dir("phase17-buildid");
    let debug_dir = dir.join("symbols");
    fs::create_dir_all(debug_dir.join(".build-id").join("ab")).unwrap();
    let elf_path = dir.join("sample.elf");
    let sidecar_path = debug_dir.join(".build-id").join("ab").join("cdef.debug");
    fs::write(&elf_path, build_plain_elf32_with_build_id(&[0xab, 0xcd, 0xef])).unwrap();
    fs::write(&sidecar_path, build_dwarf_elf32()).unwrap();

    let options = AnalyzeOptions {
        dwarf_mode: DwarfMode::On,
        source_lines: SourceLinesMode::Lines,
        debug_file_dirs: vec![debug_dir.clone()],
        ..AnalyzeOptions::default()
    };
    let result = analyze_paths(&elf_path, None, None, &options).unwrap();

    assert!(result.debug_info.dwarf_used);
    assert_eq!(result.debug_artifact.source, DebugArtifactSource::BuildId);
    assert_eq!(result.debug_artifact.build_id.as_deref(), Some("abcdef"));

    let _ = fs::remove_file(elf_path);
    let _ = fs::remove_file(sidecar_path);
}

#[test]
fn analyze_continues_when_debug_artifact_is_missing_and_auto_mode_is_used() {
    let dir = temp_dir("phase17-missing");
    fs::create_dir_all(&dir).unwrap();
    let elf_path = dir.join("sample.elf");
    fs::write(&elf_path, build_plain_elf32_with_debuglink("missing.debug")).unwrap();

    let result = analyze_paths(
        &elf_path,
        None,
        None,
        &AnalyzeOptions {
            dwarf_mode: DwarfMode::Auto,
            source_lines: SourceLinesMode::Lines,
            debug_file_dirs: vec![dir.clone()],
            ..AnalyzeOptions::default()
        },
    )
    .unwrap();

    assert!(!result.debug_info.dwarf_used);
    assert!(result.warnings.iter().any(|item| item.code == "DEBUG_ARTIFACT_NOT_FOUND"));

    let _ = fs::remove_file(elf_path);
    let _ = fs::remove_dir(dir);
}

#[test]
fn analyze_uses_split_dwo_sidecar_when_altlink_is_present() {
    let dir = temp_dir("phase17-split-dwo");
    fs::create_dir_all(&dir).unwrap();
    let elf_path = dir.join("sample.elf");
    let sidecar_path = dir.join("sample.dwo");
    fs::write(&elf_path, build_plain_elf32_with_debugaltlink("sample.dwo")).unwrap();
    fs::write(&sidecar_path, build_dwarf_elf32()).unwrap();

    let result = analyze_paths(
        &elf_path,
        None,
        None,
        &AnalyzeOptions {
            dwarf_mode: DwarfMode::On,
            source_lines: SourceLinesMode::Lines,
            debug_file_dirs: vec![dir.clone()],
            ..AnalyzeOptions::default()
        },
    )
    .unwrap();

    assert!(result.debug_info.dwarf_used);
    assert!(result.debug_info.split_dwarf_detected);
    assert_eq!(result.debug_artifact.kind, DebugArtifactKind::SplitDwo);
    assert_eq!(result.debug_artifact.source, DebugArtifactSource::UserDir);
    assert!(result.source_files.iter().any(|item| item.path.ends_with("src/main.c")));

    let _ = fs::remove_file(elf_path);
    let _ = fs::remove_file(sidecar_path);
    let _ = fs::remove_dir(dir);
}

#[test]
fn analyze_keeps_running_when_debuginfod_lookup_is_unavailable() {
    let dir = temp_dir("phase17-debuginfod-auto");
    fs::create_dir_all(&dir).unwrap();
    let elf_path = dir.join("sample.elf");
    fs::write(&elf_path, build_plain_elf32()).unwrap();

    let result = analyze_paths(
        &elf_path,
        None,
        None,
        &AnalyzeOptions {
            dwarf_mode: DwarfMode::Auto,
            source_lines: SourceLinesMode::Lines,
            debuginfod: fwmap::model::DebuginfodMode::On,
            debuginfod_urls: vec!["https://debuginfod.example.invalid".to_string()],
            debuginfod_cache_dir: Some(dir.join("cache")),
            debug_trace: true,
            ..AnalyzeOptions::default()
        },
    )
    .unwrap();

    assert!(!result.debug_info.dwarf_used);
    assert_eq!(result.debug_artifact.source, DebugArtifactSource::Debuginfod);
    assert!(result
        .debug_artifact
        .resolution_steps
        .iter()
        .any(|step| step.contains("debuginfod lookup is not implemented yet")));

    let _ = fs::remove_file(elf_path);
    let _ = fs::remove_dir_all(dir);
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("fwmap-{label}-{nanos}"))
}

fn build_plain_elf32_with_debuglink(debug_name: &str) -> Vec<u8> {
    let shstrtab = b"\0.shstrtab\0.text\0.gnu_debuglink\0";
    let debuglink = build_debuglink_section(debug_name);
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
    write_u16(&mut data, 48, 4);
    write_u16(&mut data, 50, 1);

    let shdr = 0x80usize;
    write_shdr32(&mut data, shdr + 40, 1, 3, 0, 0, 0x180, shstrtab.len() as u32, 0, 0, 1, 0);
    write_shdr32(&mut data, shdr + 80, 11, 1, 0x6, 0x1000, 0x1c0, 8, 0, 0, 4, 0);
    write_shdr32(
        &mut data,
        shdr + 120,
        17,
        1,
        0,
        0,
        0x1d0,
        debuglink.len() as u32,
        0,
        0,
        4,
        0,
    );
    data[0x180..0x180 + shstrtab.len()].copy_from_slice(shstrtab);
    data[0x1c0..0x1c8].copy_from_slice(&[0x00, 0xbf, 0x00, 0x20, 0x00, 0xbf, 0x00, 0xbf]);
    data[0x1d0..0x1d0 + debuglink.len()].copy_from_slice(&debuglink);
    data
}

fn build_plain_elf32_with_build_id(build_id: &[u8]) -> Vec<u8> {
    let shstrtab = b"\0.shstrtab\0.text\0.note.gnu.build-id\0";
    let note = build_build_id_note(build_id);
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
    write_u16(&mut data, 48, 4);
    write_u16(&mut data, 50, 1);

    let shdr = 0x80usize;
    write_shdr32(&mut data, shdr + 40, 1, 3, 0, 0, 0x180, shstrtab.len() as u32, 0, 0, 1, 0);
    write_shdr32(&mut data, shdr + 80, 11, 1, 0x6, 0x1000, 0x1c0, 8, 0, 0, 4, 0);
    write_shdr32(
        &mut data,
        shdr + 120,
        17,
        7,
        0,
        0,
        0x1d0,
        note.len() as u32,
        0,
        0,
        4,
        0,
    );
    data[0x180..0x180 + shstrtab.len()].copy_from_slice(shstrtab);
    data[0x1c0..0x1c8].copy_from_slice(&[0x00, 0xbf, 0x00, 0x20, 0x00, 0xbf, 0x00, 0xbf]);
    data[0x1d0..0x1d0 + note.len()].copy_from_slice(&note);
    data
}

fn build_plain_elf32_with_debugaltlink(debug_name: &str) -> Vec<u8> {
    let shstrtab = b"\0.shstrtab\0.text\0.gnu_debugaltlink\0";
    let debugaltlink = build_debugaltlink_section(debug_name);
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
    write_u16(&mut data, 48, 4);
    write_u16(&mut data, 50, 1);

    let shdr = 0x80usize;
    write_shdr32(&mut data, shdr + 40, 1, 3, 0, 0, 0x180, shstrtab.len() as u32, 0, 0, 1, 0);
    write_shdr32(&mut data, shdr + 80, 11, 1, 0x6, 0x1000, 0x1c0, 8, 0, 0, 4, 0);
    write_shdr32(
        &mut data,
        shdr + 120,
        17,
        1,
        0,
        0,
        0x1d0,
        debugaltlink.len() as u32,
        0,
        0,
        4,
        0,
    );
    data[0x180..0x180 + shstrtab.len()].copy_from_slice(shstrtab);
    data[0x1c0..0x1c8].copy_from_slice(&[0x00, 0xbf, 0x00, 0x20, 0x00, 0xbf, 0x00, 0xbf]);
    data[0x1d0..0x1d0 + debugaltlink.len()].copy_from_slice(&debugaltlink);
    data
}

fn build_plain_elf32() -> Vec<u8> {
    let shstrtab = b"\0.shstrtab\0.text\0";
    let mut data = vec![0u8; 0x240];
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
    write_shdr32(&mut data, shdr + 40, 1, 3, 0, 0, 0x180, shstrtab.len() as u32, 0, 0, 1, 0);
    write_shdr32(&mut data, shdr + 80, 11, 1, 0x6, 0x1000, 0x1c0, 8, 0, 0, 4, 0);
    data[0x180..0x180 + shstrtab.len()].copy_from_slice(shstrtab);
    data[0x1c0..0x1c8].copy_from_slice(&[0x00, 0xbf, 0x00, 0x20, 0x00, 0xbf, 0x00, 0xbf]);
    data
}

fn build_debuglink_section(name: &str) -> Vec<u8> {
    let mut bytes = name.as_bytes().to_vec();
    bytes.push(0);
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes
}

fn build_debugaltlink_section(name: &str) -> Vec<u8> {
    let mut bytes = name.as_bytes().to_vec();
    bytes.push(0);
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
    bytes.extend_from_slice(&[0u8; 20]);
    bytes
}

fn build_build_id_note(build_id: &[u8]) -> Vec<u8> {
    let mut note = Vec::new();
    note.extend_from_slice(&(4u32).to_le_bytes());
    note.extend_from_slice(&(build_id.len() as u32).to_le_bytes());
    note.extend_from_slice(&(3u32).to_le_bytes());
    note.extend_from_slice(b"GNU\0");
    note.extend_from_slice(build_id);
    while note.len() % 4 != 0 {
        note.push(0);
    }
    note
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

fn build_debug_abbrev() -> Vec<u8> {
    vec![0x01, 0x11, 0x00, 0x03, 0x08, 0x1b, 0x08, 0x10, 0x17, 0x00, 0x00, 0x00]
}

fn build_debug_info() -> Vec<u8> {
    let mut body = Vec::new();
    body.push(0x01);
    body.extend_from_slice(b"main.c\0");
    body.extend_from_slice(b"src\0");
    body.extend_from_slice(&0u32.to_le_bytes());

    let mut info = Vec::new();
    info.extend_from_slice(&((2 + 4 + 1 + body.len()) as u32).to_le_bytes());
    info.extend_from_slice(&4u16.to_le_bytes());
    info.extend_from_slice(&0u32.to_le_bytes());
    info.push(4);
    info.extend_from_slice(&body);
    info
}

fn build_debug_line() -> Vec<u8> {
    let mut header = vec![1, 1, 1, 0xfb, 14, 13];
    header.extend_from_slice(&[0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1]);
    header.extend_from_slice(b"src\0");
    header.push(0);
    header.extend_from_slice(b"main.c\0");
    header.extend_from_slice(&[1, 0, 0]);
    header.push(0);

    let mut program = Vec::new();
    program.extend_from_slice(&[0, 5, 2]);
    program.extend_from_slice(&0x1000u32.to_le_bytes());
    program.push(1);
    program.extend_from_slice(&[2, 4]);
    program.extend_from_slice(&[3, 9]);
    program.push(1);
    program.extend_from_slice(&[2, 4]);
    program.extend_from_slice(&[0, 1, 1]);

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
