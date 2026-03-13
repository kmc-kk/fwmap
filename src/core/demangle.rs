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
    if !matches!(mode, DemangleMode::On) && !looks_like_itanium(name) {
        return None;
    }

    parse_itanium(name)
}

pub fn display_name(symbol: &SymbolInfo) -> &str {
    symbol.demangled_name.as_deref().unwrap_or(&symbol.name)
}

fn looks_like_itanium(name: &str) -> bool {
    name.starts_with("_Z")
}

fn parse_itanium(name: &str) -> Option<String> {
    let mut rest = name.strip_prefix("_Z")?;
    let mut parts = Vec::new();

    if let Some(stripped) = rest.strip_prefix('N') {
        rest = stripped;
        while !rest.is_empty() && !rest.starts_with('E') {
            let (part, next) = parse_length_prefixed(rest)?;
            parts.push(part);
            rest = next;
        }
        rest = rest.strip_prefix('E')?;
    } else {
        let (part, next) = parse_length_prefixed(rest)?;
        parts.push(part);
        rest = next;
    }

    let suffix = if rest == "v" {
        "()"
    } else if rest.is_empty() {
        ""
    } else {
        ""
    };

    Some(format!("{}{}", parts.join("::"), suffix))
}

fn parse_length_prefixed(input: &str) -> Option<(String, &str)> {
    let digits_len = input.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits_len == 0 {
        return None;
    }
    let (digits, tail) = input.split_at(digits_len);
    let len = digits.parse::<usize>().ok()?;
    if tail.len() < len {
        return None;
    }
    let (part, next) = tail.split_at(len);
    Some((part.to_string(), next))
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
}
