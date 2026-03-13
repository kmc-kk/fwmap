use std::collections::BTreeMap;

use crate::demangle::demangle_symbol;
use crate::model::{CppAggregate, CppGroupBy, CppSymbolKind, CppSymbolSummary, CppView, SymbolInfo};

pub fn build_cpp_view(symbols: &[SymbolInfo]) -> CppView {
    let mut classified = symbols.iter().filter_map(classify_symbol).collect::<Vec<_>>();
    classified.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.display_name.cmp(&b.display_name)));

    CppView {
        top_namespaces: aggregate(&classified, |item| item.namespace.clone()),
        top_classes: aggregate(&classified, |item| item.class_name.clone()),
        top_method_families: aggregate(&classified, |item| item.method_family.clone()),
        top_template_families: aggregate(&classified, |item| item.template_family.clone()),
        runtime_overhead: aggregate(&classified, |item| runtime_bucket(item.kind)),
        lambda_groups: aggregate(&classified, |item| {
            item.lambda_related.then(|| {
                item.class_name
                    .clone()
                    .or_else(|| item.namespace.clone())
                    .unwrap_or_else(|| "(global lambda scope)".to_string())
            })
        }),
        classified_symbols: classified,
    }
}

pub fn aggregate_group_sizes(view: &CppView, group_by: CppGroupBy) -> Vec<(String, u64)> {
    let mut totals = BTreeMap::<String, u64>::new();
    for symbol in &view.classified_symbols {
        if let Some(key) = group_key(symbol, group_by) {
            *totals.entry(key).or_default() += symbol.size;
        }
    }
    totals.into_iter().collect()
}

pub fn top_group_symbols(view: &CppView, group_by: CppGroupBy, name: &str, limit: usize) -> Vec<String> {
    let mut members = view
        .classified_symbols
        .iter()
        .filter(|item| group_key(item, group_by).as_deref() == Some(name))
        .map(|item| (item.display_name.clone(), item.size))
        .collect::<Vec<_>>();
    members.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    members.into_iter().take(limit).map(|item| item.0).collect()
}

pub fn group_symbols<'a>(view: &'a CppView, group_by: CppGroupBy, name: &str) -> Vec<&'a CppSymbolSummary> {
    let mut members = view
        .classified_symbols
        .iter()
        .filter(|item| group_key(item, group_by).as_deref() == Some(name))
        .collect::<Vec<_>>();
    members.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.display_name.cmp(&b.display_name)));
    members
}

pub fn classify_symbol(symbol: &SymbolInfo) -> Option<CppSymbolSummary> {
    let display = symbol
        .demangled_name
        .clone()
        .or_else(|| demangle_symbol(&symbol.name, crate::model::DemangleMode::On))
        .unwrap_or_else(|| symbol.name.clone());
    let raw = symbol.name.clone();
    let demangled_bare = symbol.demangled_name.as_deref().map(strip_parameters);
    if symbol.demangled_name.is_none() && !looks_like_cpp_raw(&raw) {
        return None;
    }

    let mut kind = classify_kind(&display, &raw);
    let normalized = strip_known_cpp_prefix(&display).unwrap_or_else(|| strip_known_cpp_raw_prefix(&raw));
    let lambda_related = display.contains("lambda") || raw.contains("Ul") || raw.contains("MUl");
    let anonymous_namespace = display.contains("(anonymous namespace)");
    let template_instantiation = normalized.contains('<');
    let bare_name = strip_parameters(&normalized);
    let scope = split_scope(bare_name);

    let mut namespace = None;
    let mut class_name = None;
    let mut method_family = None;
    let mut template_family = find_template_family_from_name(bare_name).or_else(|| find_template_family(&scope, bare_name));

    if matches!(kind, CppSymbolKind::Vtable | CppSymbolKind::Typeinfo | CppSymbolKind::GuardVariable) {
        if !scope.is_empty() {
            class_name = Some(scope.join("::"));
            namespace = scope.len().gt(&1).then(|| scope[..scope.len() - 1].join("::"));
        }
    } else if kind == CppSymbolKind::Thunk {
        let thunk_scope = split_scope(strip_parameters(&normalized));
        let (ns, class, family) = classify_scope_members(&thunk_scope);
        namespace = ns;
        class_name = class;
        method_family = family;
    } else {
        let (ns, class, family) = classify_scope_members(&scope);
        namespace = ns;
        class_name = class;
        method_family = family;
        if matches!(kind, CppSymbolKind::Other) {
            kind = classify_callable_kind_from_name(bare_name, &scope);
            if let Some(demangled) = demangled_bare {
                let demangled_scope = split_scope(demangled);
                let demangled_kind = classify_callable_kind(&demangled_scope);
                if demangled_kind != CppSymbolKind::Function || demangled.contains("::") {
                    kind = demangled_kind;
                }
            }
        }
    }

    if class_name.is_none() {
        if let Some(demangled) = demangled_bare {
            class_name = match kind {
                CppSymbolKind::Constructor | CppSymbolKind::Destructor | CppSymbolKind::Method | CppSymbolKind::Thunk => {
                    demangled.rfind("::").map(|index| demangled[..index].to_string())
                }
                CppSymbolKind::Vtable | CppSymbolKind::Typeinfo | CppSymbolKind::GuardVariable => Some(demangled.to_string()),
                _ => None,
            };
        }
        if namespace.is_none() {
            namespace = class_name
                .as_deref()
                .and_then(|item| item.rsplit_once("::").map(|(prefix, _)| prefix.to_string()));
        }
    }
    if class_name.is_none() {
        class_name = fallback_class_name(bare_name, kind);
        if namespace.is_none() {
            namespace = class_name
                .as_deref()
                .and_then(|item| item.rsplit_once("::").map(|(prefix, _)| prefix.to_string()));
        }
    }
    if template_family.is_none() {
        if let Some(demangled) = demangled_bare {
            template_family = find_template_family_from_name(demangled).or_else(|| {
                let mut prefix = Vec::new();
                for part in split_scope(demangled) {
                    let has_template = part.contains('<');
                    prefix.push(normalize_templates(&part));
                    if has_template {
                        break;
                    }
                }
                (!prefix.is_empty() && prefix.iter().any(|item| item.contains("<...>"))).then(|| prefix.join("::"))
            });
        }
    }
    if template_family.is_none() {
        template_family = bare_name.contains('<').then(|| {
            let mut prefix = Vec::new();
            for part in split_scope(bare_name) {
                let has_template = part.contains('<');
                prefix.push(normalize_templates(&part));
                if has_template {
                    break;
                }
            }
            prefix.join("::")
        });
    }

    Some(CppSymbolSummary {
        raw_name: raw,
        display_name: display,
        kind,
        namespace,
        class_name,
        method_family,
        template_family,
        lambda_related,
        anonymous_namespace,
        template_instantiation,
        size: symbol.size,
    })
}

fn aggregate<F>(symbols: &[CppSymbolSummary], key_fn: F) -> Vec<CppAggregate>
where
    F: Fn(&CppSymbolSummary) -> Option<String>,
{
    let mut totals = BTreeMap::<String, (u64, usize)>::new();
    for item in symbols {
        if let Some(key) = key_fn(item) {
            let entry = totals.entry(key).or_default();
            entry.0 += item.size;
            entry.1 += 1;
        }
    }
    let mut rows = totals
        .into_iter()
        .map(|(name, (size, symbol_count))| CppAggregate {
            name,
            size,
            symbol_count,
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    rows.truncate(20);
    rows
}

fn group_key(symbol: &CppSymbolSummary, group_by: CppGroupBy) -> Option<String> {
    match group_by {
        CppGroupBy::Symbol => Some(symbol.display_name.clone()),
        CppGroupBy::CppTemplateFamily => symbol.template_family.clone(),
        CppGroupBy::CppClass => symbol.class_name.clone(),
        CppGroupBy::CppRuntimeOverhead => runtime_bucket(symbol.kind),
        CppGroupBy::CppLambdaGroup => symbol.lambda_related.then(|| {
            symbol
                .class_name
                .clone()
                .or_else(|| symbol.namespace.clone())
                .unwrap_or_else(|| "(global lambda scope)".to_string())
        }),
    }
}

fn runtime_bucket(kind: CppSymbolKind) -> Option<String> {
    match kind {
        CppSymbolKind::Vtable => Some("vtable".to_string()),
        CppSymbolKind::Typeinfo => Some("typeinfo".to_string()),
        CppSymbolKind::GuardVariable => Some("guard_variable".to_string()),
        CppSymbolKind::Thunk => Some("thunk".to_string()),
        _ => None,
    }
}

fn looks_like_cpp_raw(raw: &str) -> bool {
    raw.starts_with("_Z")
        || raw.starts_with("_ZT")
        || raw.starts_with("_ZGV")
        || raw.starts_with("_ZTV")
        || raw.starts_with("_ZTI")
}

fn classify_kind(display: &str, raw: &str) -> CppSymbolKind {
    if display.starts_with("vtable for ") || raw.starts_with("_ZTV") {
        CppSymbolKind::Vtable
    } else if display.starts_with("typeinfo for ")
        || display.starts_with("typeinfo name for ")
        || raw.starts_with("_ZTI")
        || raw.starts_with("_ZTS")
    {
        CppSymbolKind::Typeinfo
    } else if display.starts_with("guard variable for ") || raw.starts_with("_ZGV") {
        CppSymbolKind::GuardVariable
    } else if display.starts_with("virtual thunk to ")
        || display.starts_with("non-virtual thunk to ")
        || display.starts_with("thunk to ")
    {
        CppSymbolKind::Thunk
    } else if raw.contains("C1") || raw.contains("C2") || raw.contains("C3") {
        CppSymbolKind::Constructor
    } else if raw.contains("D0") || raw.contains("D1") || raw.contains("D2") {
        CppSymbolKind::Destructor
    } else {
        CppSymbolKind::Other
    }
}

fn strip_known_cpp_prefix(display: &str) -> Option<String> {
    for prefix in [
        "vtable for ",
        "typeinfo for ",
        "typeinfo name for ",
        "guard variable for ",
        "virtual thunk to ",
        "non-virtual thunk to ",
        "thunk to ",
    ] {
        if let Some(rest) = display.strip_prefix(prefix) {
            return Some(rest.to_string());
        }
    }
    None
}

fn strip_known_cpp_raw_prefix(raw: &str) -> String {
    raw.to_string()
}

fn classify_scope_members(scope: &[String]) -> (Option<String>, Option<String>, Option<String>) {
    if scope.is_empty() {
        return (None, None, None);
    }

    let callable_kind = classify_callable_kind(scope);
    match callable_kind {
        CppSymbolKind::Constructor | CppSymbolKind::Destructor | CppSymbolKind::Method => {
            let class_name = Some(scope[..scope.len() - 1].join("::"));
            let namespace = (scope.len() > 2).then(|| scope[..scope.len() - 2].join("::"));
            let method_family = Some(normalize_templates(&scope.join("::")));
            (namespace, class_name, method_family)
        }
        CppSymbolKind::Function => {
            let namespace = (scope.len() > 1).then(|| scope[..scope.len() - 1].join("::"));
            let method_family = Some(normalize_templates(&scope.join("::")));
            (namespace, None, method_family)
        }
        _ => (None, None, None),
    }
}

fn classify_callable_kind(scope: &[String]) -> CppSymbolKind {
    if scope.is_empty() {
        return CppSymbolKind::Other;
    }
    if scope.len() == 1 {
        return CppSymbolKind::Function;
    }

    let last = scope.last().map(|item| strip_templates(item)).unwrap_or_default();
    let prev = scope
        .get(scope.len().saturating_sub(2))
        .map(|item| strip_templates(item))
        .unwrap_or_default();

    if !prev.is_empty() && last == prev {
        return CppSymbolKind::Constructor;
    }
    if !prev.is_empty() && last == format!("~{prev}") {
        return CppSymbolKind::Destructor;
    }
    if looks_like_type_component(scope[scope.len() - 2].as_str()) {
        return CppSymbolKind::Method;
    }
    CppSymbolKind::Function
}

fn classify_callable_kind_from_name(bare_name: &str, scope: &[String]) -> CppSymbolKind {
    let by_scope = classify_callable_kind(scope);
    if by_scope != CppSymbolKind::Function {
        return by_scope;
    }
    let parts = bare_name.split("::").collect::<Vec<_>>();
    if parts.len() >= 2 {
        let last = strip_templates(parts[parts.len() - 1]);
        let prev = strip_templates(parts[parts.len() - 2]);
        if !prev.is_empty() && last == prev {
            return CppSymbolKind::Constructor;
        }
        if !prev.is_empty() && last == format!("~{prev}") {
            return CppSymbolKind::Destructor;
        }
    }
    by_scope
}

fn looks_like_type_component(component: &str) -> bool {
    let bare = strip_templates(component);
    bare.chars().next().map(|ch| ch.is_ascii_uppercase()).unwrap_or(false) || component.contains('<')
}

fn strip_parameters(name: &str) -> &str {
    let mut depth = 0usize;
    for (idx, ch) in name.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            '(' if depth == 0 => return &name[..idx],
            _ => {}
        }
    }
    name
}

fn split_scope(name: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let chars = name.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        match chars[index] {
            '<' => {
                depth += 1;
                current.push('<');
            }
            '>' => {
                depth = depth.saturating_sub(1);
                current.push('>');
            }
            ':' if depth == 0 && chars.get(index + 1) == Some(&':') => {
                if !current.is_empty() {
                    parts.push(current.clone());
                    current.clear();
                }
                index += 1;
            }
            ch => current.push(ch),
        }
        index += 1;
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

fn normalize_templates(input: &str) -> String {
    let mut output = String::new();
    let chars = input.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == '<' {
            output.push_str("<...>");
            index += 1;
            let mut depth = 1usize;
            while index < chars.len() && depth > 0 {
                match chars[index] {
                    '<' => depth += 1,
                    '>' => depth = depth.saturating_sub(1),
                    _ => {}
                }
                index += 1;
            }
            continue;
        }
        output.push(chars[index]);
        index += 1;
    }
    output
}

fn strip_templates(input: &str) -> String {
    let normalized = normalize_templates(input);
    normalized.replace("<...>", "")
}

fn find_template_family(scope: &[String], bare_name: &str) -> Option<String> {
    for index in 0..scope.len() {
        if scope[index].contains('<') {
            return Some(scope[..=index].iter().map(|item| normalize_templates(item)).collect::<Vec<_>>().join("::"));
        }
    }
    bare_name.contains('<').then(|| normalize_templates(bare_name))
}

fn find_template_family_from_name(bare_name: &str) -> Option<String> {
    let start = bare_name.find('<')?;
    let mut depth = 0usize;
    for (offset, ch) in bare_name.char_indices().skip(start) {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(normalize_templates(&bare_name[..=offset]));
                }
            }
            _ => {}
        }
    }
    None
}

fn fallback_class_name(bare_name: &str, kind: CppSymbolKind) -> Option<String> {
    let parts = bare_name.split("::").collect::<Vec<_>>();
    match kind {
        CppSymbolKind::Constructor | CppSymbolKind::Destructor | CppSymbolKind::Method | CppSymbolKind::Thunk => {
            (parts.len() >= 2).then(|| parts[..parts.len() - 1].join("::"))
        }
        CppSymbolKind::Vtable | CppSymbolKind::Typeinfo | CppSymbolKind::GuardVariable => {
            (!parts.is_empty()).then(|| parts.join("::"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{build_cpp_view, classify_symbol};
    use crate::model::{CppSymbolKind, SymbolInfo};

    fn symbol(name: &str, demangled_name: Option<&str>, size: u64) -> SymbolInfo {
        SymbolInfo {
            name: name.to_string(),
            demangled_name: demangled_name.map(str::to_string),
            section_name: Some(".text".to_string()),
            object_path: Some("main.o".to_string()),
            addr: 0,
            size,
        }
    }

    #[test]
    fn classifies_runtime_and_template_related_symbols() {
        let ctor = classify_symbol(&symbol("_ZN3app3FooC1Ev", Some("app::Foo::Foo()"), 24)).unwrap();
        assert_eq!(ctor.kind, CppSymbolKind::Constructor);
        assert_eq!(ctor.class_name.as_deref(), Some("app::Foo"));

        let vtable = classify_symbol(&symbol("_ZTVN3app3FooE", Some("vtable for app::Foo"), 40)).unwrap();
        assert_eq!(vtable.kind, CppSymbolKind::Vtable);
        assert_eq!(vtable.class_name.as_deref(), Some("app::Foo"));

        let templ = classify_symbol(&symbol(
            "_ZNSt6vectorIiE9push_backERKi",
            Some("std::vector<int>::push_back(int const&)"),
            64,
        ))
        .unwrap();
        assert_eq!(templ.kind, CppSymbolKind::Method);
        assert_eq!(templ.template_family.as_deref(), Some("std::vector<...>"));
    }

    #[test]
    fn groups_classes_templates_and_runtime_overhead() {
        let view = build_cpp_view(&[
            symbol("_ZN3app3Foo3barEv", Some("app::Foo::bar()"), 50),
            symbol("_ZN3app3FooC1Ev", Some("app::Foo::Foo()"), 30),
            symbol("_ZTVN3app3FooE", Some("vtable for app::Foo"), 20),
            symbol(
                "_ZNSt6vectorIiE9push_backERKi",
                Some("std::vector<int>::push_back(int const&)"),
                40,
            ),
            symbol(
                "_ZZN3app3Foo3barEvENKUlifE_clEif",
                Some("app::Foo::bar()::{lambda(int, float)#1}::operator()(int, float)"),
                10,
            ),
        ]);

        assert_eq!(view.top_classes.first().map(|item| item.name.as_str()), Some("app::Foo"));
        assert_eq!(
            view.top_template_families.first().map(|item| item.name.as_str()),
            Some("std::vector<...>")
        );
        assert_eq!(view.runtime_overhead.first().map(|item| item.name.as_str()), Some("vtable"));
        assert_eq!(view.lambda_groups.first().map(|item| item.symbol_count), Some(1));
    }

    #[test]
    fn ignores_plain_c_symbols_without_demangled_cpp_context() {
        assert!(classify_symbol(&symbol("main", None, 16)).is_none());
    }
}
