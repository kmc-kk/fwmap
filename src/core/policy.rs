use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{
    AppliedWaiver, AnalysisResult, DiffResult, EffectivePolicySummary, ExpiredWaiver, PolicyBudget, PolicyBudgetSnapshot,
    PolicyConfigFile, PolicyEvaluation, PolicyMatchSpec, PolicyOwnerConfidence, PolicyOwnerResolution, PolicyOwnerRule,
    PolicyOwnerSource, PolicyProfile, PolicyViolation, RuleSeverityConfig, WarningItem, WarningLevel, WarningSource,
};

pub fn load_policy_config(path: &Path) -> Result<PolicyConfigFile, String> {
    let content =
        fs::read_to_string(path).map_err(|err| format!("failed to read policy file '{}': {err}", path.display()))?;
    let config = toml::from_str::<PolicyConfigFile>(&content)
        .map_err(|err| format!("failed to parse policy file '{}': {err}", path.display()))?;
    validate_policy_config(&config)?;
    Ok(config)
}

pub fn evaluate_policy(
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    config: &PolicyConfigFile,
    requested_profile: Option<&str>,
) -> Result<PolicyEvaluation, String> {
    let (profile_name, profile) = select_profile(config, requested_profile)?;
    let mut evaluation = PolicyEvaluation {
        profile: profile_name,
        effective: effective_summary(config, &profile),
        owners: collect_owner_resolutions(current, &config.owners),
        violations: Vec::new(),
        waived: Vec::new(),
        expired_waivers: Vec::new(),
    };

    evaluate_region_budgets(current, &profile, &config.owners, &mut evaluation);
    evaluate_path_budgets(current, diff, &profile, &config.owners, &mut evaluation);
    evaluate_library_budgets(current, diff, &profile, &config.owners, &mut evaluation);
    evaluate_cpp_class_budgets(current, diff, &profile, &config.owners, &mut evaluation);
    evaluate_cpp_template_budgets(current, diff, &profile, &config.owners, &mut evaluation);

    let pending = std::mem::take(&mut evaluation.violations);
    for violation in pending {
        match apply_waiver(&violation, &config.waivers) {
            WaiverDecision::None => evaluation.violations.push(violation),
            WaiverDecision::Applied(waiver) => {
                let mut waived = violation;
                waived.waiver = Some(waiver);
                evaluation.waived.push(waived);
            }
            WaiverDecision::Expired(expired) => {
                evaluation.expired_waivers.push(expired);
                evaluation.violations.push(violation);
            }
        }
    }
    Ok(evaluation)
}

pub fn policy_warnings(evaluation: &PolicyEvaluation) -> Vec<WarningItem> {
    let mut items = evaluation
        .violations
        .iter()
        .map(|violation| WarningItem {
            level: violation.level,
            code: violation.rule_id.clone(),
            message: format_policy_message(violation),
            source: WarningSource::Analyze,
            related: Some(violation.target.clone()),
        })
        .collect::<Vec<_>>();
    items.extend(evaluation.expired_waivers.iter().map(|expired| WarningItem {
        level: WarningLevel::Warn,
        code: "POLICY_WAIVER_EXPIRED".to_string(),
        message: format!(
            "Expired waiver for {} on {} (expired {}, reason: {})",
            expired.rule, expired.target, expired.expires, expired.reason
        ),
        source: WarningSource::Analyze,
        related: Some(expired.target.clone()),
    }));
    items
}

pub fn dump_effective_policy(evaluation: &PolicyEvaluation) -> String {
    format!(
        "Policy profile: {}\nBudgets: regions={} paths={} libraries={} cpp_classes={} cpp_template_families={}\nOwners: {}\nWaivers: {}\nViolations: {}\nWaived: {}\nExpired waivers: {}",
        evaluation.profile,
        evaluation.effective.region_budget_count,
        evaluation.effective.path_budget_count,
        evaluation.effective.library_budget_count,
        evaluation.effective.cpp_class_budget_count,
        evaluation.effective.cpp_template_budget_count,
        evaluation.effective.owner_rule_count,
        evaluation.effective.waiver_count,
        evaluation.violations.len(),
        evaluation.waived.len(),
        evaluation.expired_waivers.len()
    )
}

fn validate_policy_config(config: &PolicyConfigFile) -> Result<(), String> {
    if config.version != 2 {
        return Err(format!("unsupported policy version {}, expected 2", config.version));
    }
    if config.profiles.is_empty() {
        return Err("policy must define at least one profile".to_string());
    }
    for owner in &config.owners {
        if owner.owner.trim().is_empty() {
            return Err("policy owner entry must not be empty".to_string());
        }
        validate_match_spec(&owner.match_spec, "owner")?;
    }
    for waiver in &config.waivers {
        if waiver.rule.trim().is_empty() {
            return Err("policy waiver rule must not be empty".to_string());
        }
        if waiver.reason.trim().is_empty() {
            return Err(format!("policy waiver '{}' requires a reason", waiver.rule));
        }
        validate_match_spec(&waiver.match_spec, "waiver")?;
        validate_date(&waiver.expires)?;
    }
    Ok(())
}

fn validate_match_spec(spec: &PolicyMatchSpec, label: &str) -> Result<(), String> {
    let has_any = !spec.paths.is_empty()
        || !spec.objects.is_empty()
        || !spec.libraries.is_empty()
        || !spec.cpp_classes.is_empty()
        || !spec.cpp_template_families.is_empty()
        || !spec.namespaces.is_empty();
    if has_any {
        Ok(())
    } else {
        Err(format!("policy {label} match must define at least one selector"))
    }
}

fn validate_date(value: &str) -> Result<(), String> {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        return Err(format!("invalid waiver date '{value}', expected YYYY-MM-DD"));
    }
    Ok(())
}

fn select_profile(config: &PolicyConfigFile, requested_profile: Option<&str>) -> Result<(String, PolicyProfile), String> {
    if let Some(name) = requested_profile {
        let profile = config
            .profiles
            .get(name)
            .cloned()
            .ok_or_else(|| format!("policy profile '{name}' was not found"))?;
        return Ok((name.to_string(), profile));
    }
    if let Some(name) = config.default_profile.as_deref() {
        let profile = config
            .profiles
            .get(name)
            .cloned()
            .ok_or_else(|| format!("policy default_profile '{name}' was not found"))?;
        return Ok((name.to_string(), profile));
    }
    if let Some(profile) = config.profiles.get("default").cloned() {
        return Ok(("default".to_string(), profile));
    }
    let (name, profile) = config
        .profiles
        .iter()
        .next()
        .map(|(name, profile)| (name.clone(), profile.clone()))
        .ok_or_else(|| "policy must define at least one profile".to_string())?;
    Ok((name, profile))
}

fn effective_summary(config: &PolicyConfigFile, profile: &PolicyProfile) -> EffectivePolicySummary {
    EffectivePolicySummary {
        region_budget_count: profile.budgets.regions.len(),
        path_budget_count: profile.budgets.paths.len(),
        library_budget_count: profile.budgets.libraries.len(),
        cpp_class_budget_count: profile.budgets.cpp_classes.len(),
        cpp_template_budget_count: profile.budgets.cpp_template_families.len(),
        owner_rule_count: config.owners.len(),
        waiver_count: config.waivers.len(),
    }
}

fn collect_owner_resolutions(current: &AnalysisResult, owners: &[PolicyOwnerRule]) -> Vec<PolicyOwnerResolution> {
    let mut seen = BTreeSet::new();
    let mut resolved = Vec::new();
    for source in &current.source_files {
        push_owner_resolution(&mut resolved, &mut seen, resolve_owner("path", &source.path, owners), "path", &source.path);
    }
    for object in &current.object_contributions {
        push_owner_resolution(
            &mut resolved,
            &mut seen,
            resolve_owner("object", &object.object_path, owners),
            "object",
            &object.object_path,
        );
    }
    for archive in &current.archive_contributions {
        push_owner_resolution(
            &mut resolved,
            &mut seen,
            resolve_owner("library", &archive.archive_path, owners),
            "library",
            &archive.archive_path,
        );
    }
    for class in &current.cpp_view.top_classes {
        push_owner_resolution(
            &mut resolved,
            &mut seen,
            resolve_owner("cpp_class", &class.name, owners),
            "cpp_class",
            &class.name,
        );
    }
    for family in &current.cpp_view.top_template_families {
        push_owner_resolution(
            &mut resolved,
            &mut seen,
            resolve_owner("cpp_template_family", &family.name, owners),
            "cpp_template_family",
            &family.name,
        );
    }
    resolved.sort_by(|a, b| a.target_kind.cmp(&b.target_kind).then_with(|| a.target.cmp(&b.target)));
    resolved
}

fn push_owner_resolution(
    results: &mut Vec<PolicyOwnerResolution>,
    seen: &mut BTreeSet<(String, String)>,
    owner: Option<(String, PolicyOwnerSource, PolicyOwnerConfidence)>,
    target_kind: &str,
    target: &str,
) {
    let Some((owner, owner_source, owner_confidence)) = owner else {
        return;
    };
    if !seen.insert((target_kind.to_string(), target.to_string())) {
        return;
    }
    results.push(PolicyOwnerResolution {
        target_kind: target_kind.to_string(),
        target: target.to_string(),
        owner,
        owner_source,
        owner_confidence,
    });
}

fn evaluate_region_budgets(
    current: &AnalysisResult,
    profile: &PolicyProfile,
    owners: &[PolicyOwnerRule],
    evaluation: &mut PolicyEvaluation,
) {
    for (region_name, budget) in &profile.budgets.regions {
        let Some(region) = current
            .memory
            .region_summaries
            .iter()
            .find(|item| item.region_name.eq_ignore_ascii_case(region_name))
        else {
            continue;
        };
        if let Some(level) = compare_absolute(region.used, budget) {
            evaluation.violations.push(make_violation(
                absolute_rule_id("region", level),
                level,
                format!("Region {} exceeded policy budget", region.region_name),
                "region",
                &region.region_name,
                Some(region.used),
                None,
                budget,
                resolve_owner("region", &region.region_name, owners),
            ));
        }
    }
}

fn evaluate_path_budgets(
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    profile: &PolicyProfile,
    owners: &[PolicyOwnerRule],
    evaluation: &mut PolicyEvaluation,
) {
    for (pattern, budget) in &profile.budgets.paths {
        let current_bytes = current
            .source_files
            .iter()
            .filter(|item| wildcard_match(pattern, &item.path))
            .map(|item| item.size)
            .sum::<u64>();
        let delta_bytes = diff.map(|item| {
            item.source_file_diffs
                .iter()
                .filter(|entry| wildcard_match(pattern, &entry.name))
                .map(|entry| entry.delta)
                .sum::<i64>()
        });
        push_budget_violations(evaluation, "path", pattern, current_bytes, delta_bytes, budget, owners, "Path budget");
    }
}

fn evaluate_library_budgets(
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    profile: &PolicyProfile,
    owners: &[PolicyOwnerRule],
    evaluation: &mut PolicyEvaluation,
) {
    let mut current_totals = BTreeMap::<String, u64>::new();
    for item in &current.archive_contributions {
        *current_totals.entry(item.archive_path.clone()).or_default() += item.size;
    }
    for (pattern, budget) in &profile.budgets.libraries {
        let current_bytes = current_totals
            .iter()
            .filter(|(name, _)| wildcard_match(pattern, name))
            .map(|(_, size)| *size)
            .sum::<u64>();
        let delta_bytes = diff.map(|item| {
            item.archive_diffs
                .iter()
                .filter(|entry| wildcard_match(pattern, &entry.name))
                .map(|entry| entry.delta)
                .sum::<i64>()
        });
        push_budget_violations(
            evaluation,
            "library",
            pattern,
            current_bytes,
            delta_bytes,
            budget,
            owners,
            "Library budget",
        );
    }
}

fn evaluate_cpp_class_budgets(
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    profile: &PolicyProfile,
    owners: &[PolicyOwnerRule],
    evaluation: &mut PolicyEvaluation,
) {
    for (pattern, budget) in &profile.budgets.cpp_classes {
        let current_bytes = current
            .cpp_view
            .top_classes
            .iter()
            .filter(|item| wildcard_match(pattern, &item.name))
            .map(|item| item.size)
            .sum::<u64>();
        let delta_bytes = diff.map(|item| {
            item.cpp_class_diffs
                .iter()
                .filter(|entry| wildcard_match(pattern, &entry.name))
                .map(|entry| entry.delta)
                .sum::<i64>()
        });
        push_budget_violations(
            evaluation,
            "cpp_class",
            pattern,
            current_bytes,
            delta_bytes,
            budget,
            owners,
            "C++ class budget",
        );
    }
}

fn evaluate_cpp_template_budgets(
    current: &AnalysisResult,
    diff: Option<&DiffResult>,
    profile: &PolicyProfile,
    owners: &[PolicyOwnerRule],
    evaluation: &mut PolicyEvaluation,
) {
    for (pattern, budget) in &profile.budgets.cpp_template_families {
        let current_bytes = current
            .cpp_view
            .top_template_families
            .iter()
            .filter(|item| wildcard_match(pattern, &item.name))
            .map(|item| item.size)
            .sum::<u64>();
        let delta_bytes = diff.map(|item| {
            item.cpp_template_family_diffs
                .iter()
                .filter(|entry| wildcard_match(pattern, &entry.name))
                .map(|entry| entry.delta)
                .sum::<i64>()
        });
        push_budget_violations(
            evaluation,
            "cpp_template_family",
            pattern,
            current_bytes,
            delta_bytes,
            budget,
            owners,
            "C++ template family budget",
        );
    }
}

fn push_budget_violations(
    evaluation: &mut PolicyEvaluation,
    target_kind: &str,
    target: &str,
    current_bytes: u64,
    delta_bytes: Option<i64>,
    budget: &PolicyBudget,
    owners: &[PolicyOwnerRule],
    label: &str,
) {
    if let Some(level) = compare_absolute(current_bytes, budget) {
        evaluation.violations.push(make_violation(
            absolute_rule_id(target_kind, level),
            level,
            format!("{label} exceeded absolute policy budget"),
            target_kind,
            target,
            Some(current_bytes),
            delta_bytes,
            budget,
            resolve_owner(target_kind, target, owners),
        ));
    }
    if let Some(delta) = delta_bytes {
        if let Some(level) = compare_delta(delta, budget) {
            evaluation.violations.push(make_violation(
                delta_rule_id(target_kind, level),
                level,
                format!("{label} exceeded delta policy budget"),
                target_kind,
                target,
                Some(current_bytes),
                Some(delta),
                budget,
                resolve_owner(target_kind, target, owners),
            ));
        }
    }
}

fn make_violation(
    rule_id: String,
    level: WarningLevel,
    message: String,
    target_kind: &str,
    target: &str,
    current_bytes: Option<u64>,
    delta_bytes: Option<i64>,
    budget: &PolicyBudget,
    owner: Option<(String, PolicyOwnerSource, PolicyOwnerConfidence)>,
) -> PolicyViolation {
    let (owner, owner_source, owner_confidence) = match owner {
        Some((owner, source, confidence)) => (Some(owner), Some(source), Some(confidence)),
        None => (None, None, None),
    };
    PolicyViolation {
        rule_id,
        level,
        message,
        target_kind: target_kind.to_string(),
        target: target.to_string(),
        owner,
        owner_source,
        owner_confidence,
        current_bytes,
        delta_bytes,
        budget: PolicyBudgetSnapshot {
            max_bytes: budget.max_bytes,
            warn_bytes: budget.warn_bytes,
            max_delta_bytes: budget.max_delta_bytes,
            warn_delta_bytes: budget.warn_delta_bytes,
        },
        waiver: None,
    }
}

fn compare_absolute(current_bytes: u64, budget: &PolicyBudget) -> Option<WarningLevel> {
    if let Some(max_bytes) = budget.max_bytes {
        if current_bytes > max_bytes {
            return Some(WarningLevel::Error);
        }
    }
    if let Some(warn_bytes) = budget.warn_bytes {
        if current_bytes > warn_bytes {
            return Some(map_policy_severity(budget.severity).unwrap_or(WarningLevel::Warn));
        }
    }
    None
}

fn compare_delta(delta: i64, budget: &PolicyBudget) -> Option<WarningLevel> {
    if let Some(max_delta) = budget.max_delta_bytes {
        if delta > max_delta {
            return Some(WarningLevel::Error);
        }
    }
    if let Some(warn_delta) = budget.warn_delta_bytes {
        if delta > warn_delta {
            return Some(map_policy_severity(budget.severity).unwrap_or(WarningLevel::Warn));
        }
    }
    None
}

fn absolute_rule_id(target_kind: &str, level: WarningLevel) -> String {
    let suffix = if level == WarningLevel::Error { "absolute" } else { "warn" };
    format!("budget.{target_kind}.{suffix}")
}

fn delta_rule_id(target_kind: &str, level: WarningLevel) -> String {
    let suffix = if level == WarningLevel::Error { "delta" } else { "delta.warn" };
    format!("budget.{target_kind}.{suffix}")
}

fn map_policy_severity(value: Option<RuleSeverityConfig>) -> Option<WarningLevel> {
    value.map(|level| match level {
        RuleSeverityConfig::Info => WarningLevel::Info,
        RuleSeverityConfig::Warn => WarningLevel::Warn,
        RuleSeverityConfig::Error => WarningLevel::Error,
    })
}

enum WaiverDecision {
    None,
    Applied(AppliedWaiver),
    Expired(ExpiredWaiver),
}

fn apply_waiver(violation: &PolicyViolation, waivers: &[crate::model::PolicyWaiver]) -> WaiverDecision {
    for waiver in waivers {
        if waiver.rule != violation.rule_id {
            continue;
        }
        if !waiver_matches(&waiver.match_spec, &violation.target_kind, &violation.target) {
            continue;
        }
        if waiver.expires >= today_ymd() {
            return WaiverDecision::Applied(AppliedWaiver {
                rule: waiver.rule.clone(),
                reason: waiver.reason.clone(),
                expires: waiver.expires.clone(),
            });
        }
        return WaiverDecision::Expired(ExpiredWaiver {
            rule: waiver.rule.clone(),
            target: violation.target.clone(),
            reason: waiver.reason.clone(),
            expires: waiver.expires.clone(),
        });
    }
    WaiverDecision::None
}

fn waiver_matches(spec: &PolicyMatchSpec, target_kind: &str, target: &str) -> bool {
    match target_kind {
        "path" => spec.paths.iter().any(|pattern| wildcard_match(pattern, target)),
        "library" => spec.libraries.iter().any(|pattern| wildcard_match(pattern, target)),
        "cpp_class" => spec.cpp_classes.iter().any(|pattern| wildcard_match(pattern, target)),
        "cpp_template_family" => spec.cpp_template_families.iter().any(|pattern| wildcard_match(pattern, target)),
        "object" => spec.objects.iter().any(|pattern| wildcard_match(pattern, target)),
        _ => false,
    }
}

fn resolve_owner(
    target_kind: &str,
    target: &str,
    owners: &[PolicyOwnerRule],
) -> Option<(String, PolicyOwnerSource, PolicyOwnerConfidence)> {
    for owner in owners {
        let matched = match target_kind {
            "path" => owner.match_spec.paths.iter().any(|pattern| wildcard_match(pattern, target)),
            "object" => owner.match_spec.objects.iter().any(|pattern| wildcard_match(pattern, target)),
            "library" => owner.match_spec.libraries.iter().any(|pattern| wildcard_match(pattern, target)),
            "cpp_class" => owner.match_spec.cpp_classes.iter().any(|pattern| wildcard_match(pattern, target)),
            "cpp_template_family" => owner
                .match_spec
                .cpp_template_families
                .iter()
                .any(|pattern| wildcard_match(pattern, target)),
            _ => false,
        };
        if matched {
            return Some((owner.owner.clone(), PolicyOwnerSource::Policy, PolicyOwnerConfidence::High));
        }
    }
    None
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return value.starts_with(prefix);
    }
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        return value.starts_with(prefix) && value.ends_with(suffix);
    }
    value == pattern
}

fn format_policy_message(violation: &PolicyViolation) -> String {
    let mut message = violation.message.clone();
    if let Some(current_bytes) = violation.current_bytes {
        message.push_str(&format!(" | current={} bytes", current_bytes));
    }
    if let Some(delta_bytes) = violation.delta_bytes {
        message.push_str(&format!(" | delta={:+} bytes", delta_bytes));
    }
    if let Some(owner) = violation.owner.as_deref() {
        message.push_str(&format!(" | owner={owner}"));
    }
    message
}

fn today_ymd() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        / 86_400;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::{evaluate_policy, load_policy_config, policy_warnings};
    use crate::model::{
        AnalysisResult, ArchiveContribution, BinaryInfo, CppAggregate, CppView, DebugArtifactInfo, DebugInfoSummary,
        DiffChangeKind, DiffEntry, DiffResult, DiffSummary, MemoryRegion, MemorySummary, ObjectContribution,
        ObjectSourceKind, RegionUsageSummary, SectionCategory, SectionTotal, ToolchainInfo, ToolchainKind,
        ToolchainSelection, UnknownSourceBucket,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_valid_policy_v2() {
        let path = temp_file("policy-ok");
        fs::write(
            &path,
            r#"
version = 2
default_profile = "release"

[profiles.release.budgets.regions.FLASH]
max_bytes = 1024

[[owners]]
owner = "platform-team"
[owners.match]
paths = ["src/**"]
"#,
        )
        .unwrap();
        let config = load_policy_config(&path).unwrap();
        assert_eq!(config.version, 2);
        assert!(config.profiles.contains_key("release"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_invalid_policy_version() {
        let path = temp_file("policy-bad");
        fs::write(&path, "version = 1\n[profiles.release.budgets]\n").unwrap();
        let err = load_policy_config(&path).unwrap_err();
        assert!(err.contains("unsupported policy version"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn resolves_owner_by_path_and_library() {
        let current = sample_analysis();
        let config = toml::from_str(
            r#"
version = 2
default_profile = "release"

[profiles.release.budgets.paths."src/net/**"]
warn_delta_bytes = 128

[profiles.release.budgets.libraries."libcrypto.a"]
warn_delta_bytes = 128

[[owners]]
owner = "network-team"
[owners.match]
paths = ["src/net/**"]

[[owners]]
owner = "security-team"
[owners.match]
libraries = ["libcrypto.a"]
"#,
        )
        .unwrap();
        let evaluation = evaluate_policy(&current, Some(&sample_diff()), &config, Some("release")).unwrap();
        assert!(evaluation
            .owners
            .iter()
            .any(|item| item.target == "src/net/socket.cpp" && item.owner == "network-team"));
        assert!(evaluation
            .owners
            .iter()
            .any(|item| item.target == "libcrypto.a" && item.owner == "security-team"));
    }

    #[test]
    fn flags_region_and_path_budget_violations() {
        let current = sample_analysis();
        let config = toml::from_str(
            r#"
version = 2
default_profile = "release"

[profiles.release.budgets.regions.FLASH]
max_bytes = 700

[profiles.release.budgets.paths."src/net/**"]
max_delta_bytes = 256
"#,
        )
        .unwrap();
        let evaluation = evaluate_policy(&current, Some(&sample_diff()), &config, None).unwrap();
        assert!(evaluation.violations.iter().any(|item| item.rule_id == "budget.region.absolute"));
        assert!(evaluation.violations.iter().any(|item| item.rule_id == "budget.path.delta"));
    }

    #[test]
    fn applies_active_waiver_and_reports_expired_waiver() {
        let current = sample_analysis();
        let config = toml::from_str(
            r#"
version = 2
default_profile = "release"

[profiles.release.budgets.paths."src/net/**"]
max_delta_bytes = 256

[profiles.release.budgets.libraries."libcrypto.a"]
max_delta_bytes = 128

[[waivers]]
rule = "budget.path.delta"
expires = "2099-12-31"
reason = "migration"
[waivers.match]
paths = ["src/net/**"]

[[waivers]]
rule = "budget.library.delta"
expires = "2020-01-01"
reason = "temporary"
[waivers.match]
libraries = ["libcrypto.a"]
"#,
        )
        .unwrap();
        let evaluation = evaluate_policy(&current, Some(&sample_diff()), &config, None).unwrap();
        assert!(evaluation.waived.iter().any(|item| item.rule_id == "budget.path.delta"));
        assert!(evaluation
            .expired_waivers
            .iter()
            .any(|item| item.rule == "budget.library.delta"));
        let warnings = policy_warnings(&evaluation);
        assert!(warnings.iter().any(|item| item.code == "POLICY_WAIVER_EXPIRED"));
    }

    #[test]
    fn supports_multi_profile_selection() {
        let current = sample_analysis();
        let config = toml::from_str(
            r#"
version = 2

[profiles.debug.budgets.paths."src/net/**"]
warn_delta_bytes = 2048

[profiles.release.budgets.paths."src/net/**"]
warn_delta_bytes = 128
"#,
        )
        .unwrap();
        let debug_eval = evaluate_policy(&current, Some(&sample_diff()), &config, Some("debug")).unwrap();
        let release_eval = evaluate_policy(&current, Some(&sample_diff()), &config, Some("release")).unwrap();
        assert!(debug_eval.violations.is_empty());
        assert!(!release_eval.violations.is_empty());
    }

    fn temp_file(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("fwmap-{name}-{nanos}.toml"))
    }

    fn sample_analysis() -> AnalysisResult {
        AnalysisResult {
            binary: BinaryInfo {
                path: "sample.elf".to_string(),
                arch: "ARM".to_string(),
                elf_class: "ELF32".to_string(),
                endian: "little-endian".to_string(),
            },
            git: None,
            rust_context: None,
            toolchain: ToolchainInfo {
                requested: ToolchainSelection::Auto,
                detected: None,
                resolved: ToolchainKind::Gnu,
                linker_family: crate::model::LinkerFamily::Gnu,
                map_format: crate::model::MapFormat::Gnu,
                parser_warnings_count: 0,
            },
            debug_info: DebugInfoSummary::default(),
            debug_artifact: DebugArtifactInfo::default(),
            policy: None,
            sections: Vec::new(),
            symbols: Vec::new(),
            object_contributions: vec![ObjectContribution {
                object_path: "build/net.o".to_string(),
                source_kind: ObjectSourceKind::Object,
                section_name: Some(".text".to_string()),
                size: 420,
            }],
            archive_contributions: vec![ArchiveContribution {
                archive_path: "libcrypto.a".to_string(),
                member_path: Some("aes.o".to_string()),
                section_name: Some(".text".to_string()),
                size: 260,
            }],
            archive_pulls: Vec::new(),
            whole_archive_candidates: Vec::new(),
            relocation_references: Vec::new(),
            cross_references: Vec::new(),
            cpp_view: CppView {
                top_classes: vec![CppAggregate {
                    name: "net::Socket".to_string(),
                    size: 240,
                    symbol_count: 2,
                }],
                top_template_families: vec![CppAggregate {
                    name: "etl::vector<...>".to_string(),
                    size: 180,
                    symbol_count: 1,
                }],
                ..CppView::default()
            },
            linker_script: None,
            memory: MemorySummary {
                rom_bytes: 800,
                ram_bytes: 64,
                section_totals: vec![SectionTotal {
                    section_name: ".text".to_string(),
                    size: 800,
                    category: SectionCategory::Rom,
                }],
                memory_regions: vec![MemoryRegion {
                    name: "FLASH".to_string(),
                    origin: 0x0800_0000,
                    length: 1024,
                    attributes: "rx".to_string(),
                }],
                region_summaries: vec![RegionUsageSummary {
                    region_name: "FLASH".to_string(),
                    origin: 0x0800_0000,
                    length: 1024,
                    used: 800,
                    free: 224,
                    usage_ratio: 800.0 / 1024.0,
                    sections: Vec::new(),
                }],
            },
            compilation_units: Vec::new(),
            source_files: vec![crate::model::SourceFile {
                path: "src/net/socket.cpp".to_string(),
                display_path: "src/net/socket.cpp".to_string(),
                directory: "src/net".to_string(),
                size: 420,
                functions: 2,
                line_ranges: 4,
            }],
            line_attributions: Vec::new(),
            line_hotspots: Vec::new(),
            function_attributions: Vec::new(),
            unknown_source: UnknownSourceBucket::default(),
            warnings: Vec::new(),
        }
    }

    fn sample_diff() -> DiffResult {
        DiffResult {
            rom_delta: 300,
            ram_delta: 16,
            unknown_source_delta: 0,
            summary: DiffSummary::default(),
            section_diffs: Vec::new(),
            symbol_diffs: Vec::new(),
            object_diffs: Vec::new(),
            archive_diffs: vec![DiffEntry {
                name: "libcrypto.a".to_string(),
                current: 260,
                previous: 80,
                delta: 180,
                change: DiffChangeKind::Increased,
            }],
            source_file_diffs: vec![DiffEntry {
                name: "src/net/socket.cpp".to_string(),
                current: 420,
                previous: 80,
                delta: 340,
                change: DiffChangeKind::Increased,
            }],
            function_diffs: Vec::new(),
            line_diffs: Vec::new(),
            cpp_template_family_diffs: vec![DiffEntry {
                name: "etl::vector<...>".to_string(),
                current: 180,
                previous: 60,
                delta: 120,
                change: DiffChangeKind::Increased,
            }],
            cpp_class_diffs: vec![DiffEntry {
                name: "net::Socket".to_string(),
                current: 240,
                previous: 100,
                delta: 140,
                change: DiffChangeKind::Increased,
            }],
            cpp_runtime_overhead_diffs: Vec::new(),
            cpp_lambda_group_diffs: Vec::new(),
        }
    }
}
