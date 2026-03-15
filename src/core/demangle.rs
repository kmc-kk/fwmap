use crate::model::{DemangleMode, SymbolInfo};

pub fn apply_demangling(symbols: &mut [SymbolInfo], mode: DemangleMode) {
    if matches!(mode, DemangleMode::Off) {
        for symbol in symbols {
            symbol.demangled_name = None;
        }
        return;
    }

    for symbol in symbols {
        symbol.demangled_name = demangle_symbol(&symbol.name, mode);
    }
}

pub fn demangle_symbol(name: &str, mode: DemangleMode) -> Option<String> {
    if matches!(mode, DemangleMode::Off) {
        return None;
    }
    let normalized = strip_llvm_suffix(name);
    if !matches!(mode, DemangleMode::On) && !looks_like_itanium(normalized) {
        return None;
    }

    parse_itanium(normalized).map(clean_rust_legacy_demangle)
}

pub fn display_name(symbol: &SymbolInfo) -> &str {
    symbol.demangled_name.as_deref().unwrap_or(&symbol.name)
}

fn looks_like_itanium(name: &str) -> bool {
    name.starts_with("_Z")
}

fn strip_llvm_suffix(name: &str) -> &str {
    name.split_once(".llvm.").map(|(base, _)| base).unwrap_or(name)
}

fn clean_rust_legacy_demangle(value: String) -> String {
    let mut parts = value.split("::").map(str::to_string).collect::<Vec<_>>();
    if parts.last().is_some_and(|part| looks_like_rust_hash_component(part)) {
        if let Some(hash) = parts.pop() {
            return if parts.is_empty() {
                hash
            } else {
                format!("{} [{}]", parts.join("::"), hash)
            };
        }
    }
    value
}

fn looks_like_rust_hash_component(value: &str) -> bool {
    value.len() == 17
        && value.starts_with('h')
        && value
            .chars()
            .skip(1)
            .all(|ch| ch.is_ascii_hexdigit())
}

fn parse_itanium(name: &str) -> Option<String> {
    let mut parser = ItaniumParser::new(name.strip_prefix("_Z")?);
    let path = if parser.consume('N') {
        let path = parser.parse_nested_name()?;
        parser.expect('E')?;
        path
    } else {
        vec![parser.parse_name_component()?]
    };
    let signature = parser.parse_function_suffix();
    if !parser.is_done() {
        return None;
    }
    Some(format!("{}{}", path.join("::"), signature))
}

struct ItaniumParser<'a> {
    input: &'a str,
    offset: usize,
    substitutions: Vec<String>,
}

impl<'a> ItaniumParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            offset: 0,
            substitutions: Vec::new(),
        }
    }

    fn parse_nested_name(&mut self) -> Option<Vec<String>> {
        let mut parts = Vec::new();
        while !self.peek_is('E') {
            parts.push(self.parse_name_component()?);
        }
        Some(parts)
    }

    fn parse_name_component(&mut self) -> Option<String> {
        let mut name = match self.peek_char()? {
            'S' => self.parse_substitution()?,
            c if c.is_ascii_digit() => self.parse_source_name()?,
            _ => return None,
        };

        if self.consume('I') {
            // Keep template rendering shallow but readable; this demangler is for reports, not full ABI round-tripping.
            let mut args = Vec::new();
            while !self.peek_is('E') {
                args.push(self.parse_type()?);
            }
            self.expect('E')?;
            name = format!("{name}<{}>", args.join(", "));
        }
        self.substitutions.push(name.clone());
        Some(name)
    }

    fn parse_type(&mut self) -> Option<String> {
        match self.peek_char()? {
            'P' => {
                self.consume('P');
                let inner = self.parse_type()?;
                Some(format!("{inner}*"))
            }
            'R' => {
                self.consume('R');
                let inner = self.parse_type()?;
                Some(format!("{inner}&"))
            }
            'K' => {
                self.consume('K');
                let inner = self.parse_type()?;
                Some(format!("const {inner}"))
            }
            'S' => self.parse_substitution(),
            'N' => {
                self.consume('N');
                let path = self.parse_nested_name()?;
                self.expect('E')?;
                let name = path.join("::");
                self.substitutions.push(name.clone());
                Some(name)
            }
            c if c.is_ascii_digit() => self.parse_name_component(),
            'v' => {
                self.consume('v');
                Some("void".to_string())
            }
            'b' => {
                self.consume('b');
                Some("bool".to_string())
            }
            'c' => {
                self.consume('c');
                Some("char".to_string())
            }
            'i' => {
                self.consume('i');
                Some("int".to_string())
            }
            'j' => {
                self.consume('j');
                Some("unsigned int".to_string())
            }
            'l' => {
                self.consume('l');
                Some("long".to_string())
            }
            'm' => {
                self.consume('m');
                Some("unsigned long".to_string())
            }
            _ => None,
        }
    }

    fn parse_source_name(&mut self) -> Option<String> {
        let len = self.parse_number()?;
        let tail = self.input.get(self.offset..)?;
        if tail.starts_with("_GLOBAL__N_1") && len == 12 {
            self.offset += 12;
            return Some("(anonymous namespace)".to_string());
        }
        let end = self.offset.checked_add(len)?;
        let name = self.input.get(self.offset..end)?.to_string();
        self.offset = end;
        Some(name)
    }

    fn parse_substitution(&mut self) -> Option<String> {
        self.expect('S')?;
        if self.consume('_') {
            return self.substitutions.first().cloned();
        }
        let mut seq = String::new();
        while let Some(ch) = self.peek_char() {
            if ch == '_' {
                break;
            }
            seq.push(ch);
            self.offset += ch.len_utf8();
        }
        self.expect('_')?;
        // Itanium substitutions are base-36 encoded and refer back to previously parsed names/types.
        let index = if seq.is_empty() {
            0
        } else {
            usize::from_str_radix(&seq, 36).ok()?.saturating_add(1)
        };
        self.substitutions.get(index).cloned()
    }

    fn parse_function_suffix(&mut self) -> String {
        if self.is_done() {
            return String::new();
        }
        if self.remaining() == "v" {
            self.offset += 1;
            return "()".to_string();
        }
        let start = self.offset;
        let mut args = Vec::new();
        while self.offset < self.input.len() {
            if let Some(arg) = self.parse_type() {
                if arg == "void" && self.offset == start + 1 {
                    return "()".to_string();
                }
                args.push(arg);
            } else {
                break;
            }
        }
        if args.is_empty() {
            String::new()
        } else {
            format!("({})", args.join(", "))
        }
    }

    fn parse_number(&mut self) -> Option<usize> {
        let start = self.offset;
        while let Some(ch) = self.peek_char() {
            if !ch.is_ascii_digit() {
                break;
            }
            self.offset += ch.len_utf8();
        }
        (self.offset > start)
            .then(|| self.input.get(start..self.offset))
            .flatten()?
            .parse()
            .ok()
    }

    fn expect(&mut self, ch: char) -> Option<()> {
        self.consume(ch).then_some(())
    }

    fn consume(&mut self, ch: char) -> bool {
        if self.peek_is(ch) {
            self.offset += ch.len_utf8();
            true
        } else {
            false
        }
    }

    fn peek_is(&self, ch: char) -> bool {
        self.peek_char() == Some(ch)
    }

    fn peek_char(&self) -> Option<char> {
        self.input.get(self.offset..)?.chars().next()
    }

    fn remaining(&self) -> &str {
        self.input.get(self.offset..).unwrap_or("")
    }

    fn is_done(&self) -> bool {
        self.offset >= self.input.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_demangling, demangle_symbol, display_name};
    use crate::model::{DemangleMode, SymbolInfo};

    #[test]
    fn demangles_itanium_symbol_when_enabled() {
        let value = demangle_symbol("_ZN3foo3barEv", DemangleMode::On).unwrap();
        assert!(value.contains("foo"));
        assert!(value.contains("bar"));
    }

    #[test]
    fn demangles_nested_templates_and_anonymous_namespace() {
        let value = demangle_symbol(
            "_ZN12_GLOBAL__N_116itanium_demangle22AbstractManglingParserINS0_14ManglingParserINS_16DefaultAllocatorEEES3_E16parseExprPrimaryEv",
            DemangleMode::On,
        )
        .unwrap();
        assert!(value.contains("(anonymous namespace)::itanium_demangle::AbstractManglingParser<"));
        assert!(value.contains("ManglingParser<"));
        assert!(value.contains("DefaultAllocator"));
        assert!(value.contains("parseExprPrimary()"));
    }

    #[test]
    fn demangles_pointer_and_const_arguments() {
        let value = demangle_symbol(
            "_ZN3kmg5solid3elf9ELFObject5CheckEPKcP20_SOLID_LDR_FILEINFO_",
            DemangleMode::On,
        )
        .unwrap();
        assert_eq!(
            value,
            "kmg::solid::elf::ELFObject::Check(const char*, _SOLID_LDR_FILEINFO_*)"
        );
    }

    #[test]
    fn auto_mode_skips_plain_c_symbols() {
        assert_eq!(demangle_symbol("main", DemangleMode::Auto), None);
    }

    #[test]
    fn display_name_prefers_demangled_name() {
        let mut symbols = vec![SymbolInfo {
            name: "_ZN3foo3barEv".to_string(),
            demangled_name: None,
            section_name: None,
            object_path: None,
            addr: 0,
            size: 4,
        }];
        apply_demangling(&mut symbols, DemangleMode::On);
        assert_ne!(display_name(&symbols[0]), symbols[0].name);
    }

    #[test]
    fn demangles_rust_legacy_symbol_with_llvm_suffix() {
        let value = demangle_symbol(
            "_ZN5fwmap6ingest5dwarf19parse_dwarf_enabled17h75e4fa34e7c912e2E.llvm.16657126762338766321",
            DemangleMode::On,
        )
        .unwrap();
        assert_eq!(
            value,
            "fwmap::ingest::dwarf::parse_dwarf_enabled [h75e4fa34e7c912e2]"
        );
    }
}
