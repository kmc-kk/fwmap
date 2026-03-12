use std::fs;
use std::path::Path;

use crate::model::{BinaryInfo, SectionCategory, SectionInfo, SymbolInfo};

const EI_CLASS: usize = 4;
const EI_DATA: usize = 5;
const ELFCLASS32: u8 = 1;
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const ELFDATA2MSB: u8 = 2;
const SHT_SYMTAB: u32 = 2;
const SHF_WRITE: u64 = 0x1;
const SHF_ALLOC: u64 = 0x2;
const SHF_EXECINSTR: u64 = 0x4;
const SHN_UNDEF: u16 = 0;

#[derive(Debug, Clone)]
pub struct ElfIngestResult {
    pub binary: BinaryInfo,
    pub sections: Vec<SectionInfo>,
    pub symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Clone, Copy)]
enum Endian {
    Little,
    Big,
}

#[derive(Debug, Clone)]
struct RawSection {
    name_offset: u32,
    addr: u64,
    size: u64,
    flags: u64,
    kind: u32,
    link: u32,
    entsize: u64,
    offset: u64,
}

pub fn parse_elf(path: &Path) -> Result<ElfIngestResult, String> {
    let bytes = fs::read(path).map_err(|err| format!("failed to read ELF '{}': {err}", path.display()))?;
    if bytes.len() < 16 || &bytes[0..4] != b"\x7fELF" {
        return Err(format!("'{}' is not an ELF file", path.display()));
    }

    let class = bytes[EI_CLASS];
    let endian = match bytes[EI_DATA] {
        ELFDATA2LSB => Endian::Little,
        ELFDATA2MSB => Endian::Big,
        _ => return Err(format!("unsupported ELF endianness in '{}'", path.display())),
    };

    let (elf_class, e_machine, shoff, shentsize, shnum, shstrndx) = match class {
        ELFCLASS32 => (
            "ELF32".to_string(),
            read_u16(&bytes, 18, endian)? as u32,
            read_u32(&bytes, 32, endian)? as u64,
            read_u16(&bytes, 46, endian)? as u64,
            read_u16(&bytes, 48, endian)? as u64,
            read_u16(&bytes, 50, endian)? as u64,
        ),
        ELFCLASS64 => (
            "ELF64".to_string(),
            read_u16(&bytes, 18, endian)? as u32,
            read_u64(&bytes, 40, endian)?,
            read_u16(&bytes, 58, endian)? as u64,
            read_u16(&bytes, 60, endian)? as u64,
            read_u16(&bytes, 62, endian)? as u64,
        ),
        _ => return Err(format!("unsupported ELF class in '{}'", path.display())),
    };

    let raw_sections = parse_section_headers(&bytes, class, endian, shoff, shentsize, shnum)?;
    let section_names = build_string_table(&bytes, &raw_sections, shstrndx as usize)?;
    let sections = raw_sections
        .iter()
        .map(|raw| SectionInfo {
            name: string_at(&section_names, raw.name_offset),
            addr: raw.addr,
            size: raw.size,
            flags: describe_section_flags(raw.flags),
            category: classify_section(&string_at(&section_names, raw.name_offset), raw.flags),
        })
        .collect::<Vec<_>>();
    let symbols = parse_symbols(&bytes, class, endian, &raw_sections, &section_names)?;

    Ok(ElfIngestResult {
        binary: BinaryInfo {
            path: path.display().to_string(),
            arch: machine_name(e_machine).to_string(),
            elf_class,
            endian: match endian {
                Endian::Little => "little-endian".to_string(),
                Endian::Big => "big-endian".to_string(),
            },
        },
        sections,
        symbols,
    })
}

fn parse_section_headers(
    bytes: &[u8],
    class: u8,
    endian: Endian,
    shoff: u64,
    shentsize: u64,
    shnum: u64,
) -> Result<Vec<RawSection>, String> {
    let mut sections = Vec::with_capacity(shnum as usize);
    for idx in 0..shnum {
        let base = offset_checked(shoff, shentsize, idx, bytes.len())?;
        let section = match class {
            ELFCLASS32 => RawSection {
                name_offset: read_u32(bytes, base, endian)?,
                kind: read_u32(bytes, base + 4, endian)?,
                flags: read_u32(bytes, base + 8, endian)? as u64,
                addr: read_u32(bytes, base + 12, endian)? as u64,
                offset: read_u32(bytes, base + 16, endian)? as u64,
                size: read_u32(bytes, base + 20, endian)? as u64,
                link: read_u32(bytes, base + 24, endian)?,
                entsize: read_u32(bytes, base + 36, endian)? as u64,
            },
            ELFCLASS64 => RawSection {
                name_offset: read_u32(bytes, base, endian)?,
                kind: read_u32(bytes, base + 4, endian)?,
                flags: read_u64(bytes, base + 8, endian)?,
                addr: read_u64(bytes, base + 16, endian)?,
                offset: read_u64(bytes, base + 24, endian)?,
                size: read_u64(bytes, base + 32, endian)?,
                link: read_u32(bytes, base + 40, endian)?,
                entsize: read_u64(bytes, base + 56, endian)?,
            },
            _ => unreachable!(),
        };
        sections.push(section);
    }
    Ok(sections)
}

fn parse_symbols(
    bytes: &[u8],
    class: u8,
    endian: Endian,
    sections: &[RawSection],
    section_names: &[u8],
) -> Result<Vec<SymbolInfo>, String> {
    let mut symbols = Vec::new();
    for section in sections.iter().filter(|section| section.kind == SHT_SYMTAB) {
        if section.entsize == 0 {
            continue;
        }
        let strings = build_string_table(bytes, sections, section.link as usize)?;
        let count = section.size / section.entsize;
        for idx in 0..count {
            let base = offset_checked(section.offset, section.entsize, idx, bytes.len())?;
            let (name_offset, section_index, size) = match class {
                ELFCLASS32 => (
                    read_u32(bytes, base, endian)?,
                    read_u16(bytes, base + 14, endian)?,
                    read_u32(bytes, base + 8, endian)? as u64,
                ),
                ELFCLASS64 => (
                    read_u32(bytes, base, endian)?,
                    read_u16(bytes, base + 6, endian)?,
                    read_u64(bytes, base + 16, endian)?,
                ),
                _ => unreachable!(),
            };
            if section_index == SHN_UNDEF {
                continue;
            }
            let name = string_at(&strings, name_offset);
            if name.is_empty() {
                continue;
            }
            let section_name = sections
                .get(section_index as usize)
                .map(|s| string_at(section_names, s.name_offset));
            symbols.push(SymbolInfo {
                name,
                demangled_name: None,
                section_name,
                object_path: None,
                size,
            });
        }
    }
    Ok(symbols)
}

fn build_string_table(bytes: &[u8], sections: &[RawSection], index: usize) -> Result<Vec<u8>, String> {
    let section = sections.get(index).ok_or_else(|| format!("invalid string table index {index}"))?;
    let start = section.offset as usize;
    let end = start.checked_add(section.size as usize).ok_or("string table overflow".to_string())?;
    let slice = bytes.get(start..end).ok_or("string table is out of ELF range".to_string())?;
    Ok(slice.to_vec())
}

fn string_at(table: &[u8], offset: u32) -> String {
    let start = offset as usize;
    if start >= table.len() {
        return String::new();
    }
    let end = table[start..]
        .iter()
        .position(|byte| *byte == 0)
        .map(|p| start + p)
        .unwrap_or(table.len());
    String::from_utf8_lossy(&table[start..end]).into_owned()
}

fn describe_section_flags(flags: u64) -> Vec<String> {
    let mut result = Vec::new();
    if flags & SHF_ALLOC != 0 {
        result.push("ALLOC".to_string());
    }
    if flags & SHF_WRITE != 0 {
        result.push("WRITE".to_string());
    }
    if flags & SHF_EXECINSTR != 0 {
        result.push("EXEC".to_string());
    }
    result
}

fn classify_section(name: &str, flags: u64) -> SectionCategory {
    if name == ".data" || name == ".bss" || (flags & SHF_ALLOC != 0 && flags & SHF_WRITE != 0) {
        SectionCategory::Ram
    } else if matches!(name, ".text" | ".rodata") || (flags & SHF_ALLOC != 0 && flags & SHF_WRITE == 0) {
        SectionCategory::Rom
    } else {
        SectionCategory::Other
    }
}

fn machine_name(machine: u32) -> &'static str {
    match machine {
        0x03 => "x86",
        0x28 => "ARM",
        0x3e => "x86-64",
        0xb7 => "AArch64",
        0xf3 => "RISC-V",
        _ => "Unknown",
    }
}

fn read_u16(bytes: &[u8], offset: usize, endian: Endian) -> Result<u16, String> {
    let array: [u8; 2] = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| format!("ELF truncated near offset {offset}"))?
        .try_into()
        .map_err(|_| "failed to read u16".to_string())?;
    Ok(match endian {
        Endian::Little => u16::from_le_bytes(array),
        Endian::Big => u16::from_be_bytes(array),
    })
}

fn read_u32(bytes: &[u8], offset: usize, endian: Endian) -> Result<u32, String> {
    let array: [u8; 4] = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("ELF truncated near offset {offset}"))?
        .try_into()
        .map_err(|_| "failed to read u32".to_string())?;
    Ok(match endian {
        Endian::Little => u32::from_le_bytes(array),
        Endian::Big => u32::from_be_bytes(array),
    })
}

fn read_u64(bytes: &[u8], offset: usize, endian: Endian) -> Result<u64, String> {
    let array: [u8; 8] = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| format!("ELF truncated near offset {offset}"))?
        .try_into()
        .map_err(|_| "failed to read u64".to_string())?;
    Ok(match endian {
        Endian::Little => u64::from_le_bytes(array),
        Endian::Big => u64::from_be_bytes(array),
    })
}

fn offset_checked(base: u64, size: u64, index: u64, total_len: usize) -> Result<usize, String> {
    let offset = base
        .checked_add(size.checked_mul(index).ok_or("ELF offset multiplication overflow".to_string())?)
        .ok_or("ELF offset overflow".to_string())?;
    let end = offset.checked_add(size).ok_or("ELF offset overflow".to_string())?;
    if end > total_len as u64 {
        return Err("ELF section header exceeds file size".to_string());
    }
    Ok(offset as usize)
}

#[cfg(test)]
mod tests {
    use super::parse_elf;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_minimal_elf64_fixture() {
        let path = temp_file("sample.elf");
        fs::write(&path, build_sample_elf64()).unwrap();
        let result = parse_elf(&path).unwrap();
        assert_eq!(result.binary.elf_class, "ELF64");
        assert!(result.sections.iter().any(|section| section.name == ".text"));
        assert!(result.symbols.iter().any(|symbol| symbol.name == "main"));
        let _ = fs::remove_file(path);
    }

    fn temp_file(name: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("fwmap-{nanos}-{name}"))
    }

    fn build_sample_elf64() -> Vec<u8> {
        let mut data = vec![0u8; 0x340];
        data[0..4].copy_from_slice(b"\x7fELF");
        data[4] = 2;
        data[5] = 1;
        data[6] = 1;
        write_u16(&mut data, 16, 2);
        write_u16(&mut data, 18, 0x3e);
        write_u32(&mut data, 20, 1);
        write_u64(&mut data, 40, 0x100);
        write_u16(&mut data, 52, 64);
        write_u16(&mut data, 58, 64);
        write_u16(&mut data, 60, 5);
        write_u16(&mut data, 62, 1);

        let shstrtab = b"\0.shstrtab\0.text\0.symtab\0.strtab\0";
        let strtab = b"\0main\0";

        let shdr = 0x100usize;
        write_shdr64(&mut data, shdr + 64, 1, 3, 0, 0, 0x240, shstrtab.len() as u64, 0, 0, 1, 0);
        write_shdr64(&mut data, shdr + 128, 11, 1, 0x6, 0x400000, 0x270, 4, 0, 0, 16, 0);
        write_shdr64(&mut data, shdr + 192, 17, 2, 0, 0, 0x278, 48, 4, 1, 8, 24);
        write_shdr64(&mut data, shdr + 256, 25, 3, 0, 0, 0x2b0, strtab.len() as u64, 0, 0, 1, 0);

        data[0x240..0x240 + shstrtab.len()].copy_from_slice(shstrtab);
        data[0x270..0x274].copy_from_slice(&[0xC3, 0x90, 0x90, 0x90]);
        write_sym64(&mut data, 0x278, 0, 0, 0, 0, 0, 0);
        write_sym64(&mut data, 0x290, 1, 0x12, 0, 2, 0x400000, 4);
        data[0x2b0..0x2b0 + strtab.len()].copy_from_slice(strtab);
        data
    }

    fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
        buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
        buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(buf: &mut [u8], offset: usize, value: u64) {
        buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn write_shdr64(
        buf: &mut [u8],
        offset: usize,
        name: u32,
        kind: u32,
        flags: u64,
        addr: u64,
        file_offset: u64,
        size: u64,
        link: u32,
        info: u32,
        addralign: u64,
        entsize: u64,
    ) {
        write_u32(buf, offset, name);
        write_u32(buf, offset + 4, kind);
        write_u64(buf, offset + 8, flags);
        write_u64(buf, offset + 16, addr);
        write_u64(buf, offset + 24, file_offset);
        write_u64(buf, offset + 32, size);
        write_u32(buf, offset + 40, link);
        write_u32(buf, offset + 44, info);
        write_u64(buf, offset + 48, addralign);
        write_u64(buf, offset + 56, entsize);
    }

    fn write_sym64(buf: &mut [u8], offset: usize, name: u32, info: u8, other: u8, shndx: u16, value: u64, size: u64) {
        write_u32(buf, offset, name);
        buf[offset + 4] = info;
        buf[offset + 5] = other;
        write_u16(buf, offset + 6, shndx);
        write_u64(buf, offset + 8, value);
        write_u64(buf, offset + 16, size);
    }
}
