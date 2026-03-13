use std::fs;
use std::path::Path;

use crate::model::{CustomRule, RuleConfigFile, RuleKind, RuleThresholdOverrides, ThresholdConfig};

pub fn load_rule_config(path: &Path) -> Result<RuleConfigFile, String> {
    let content =
        fs::read_to_string(path).map_err(|err| format!("failed to read rule file '{}': {err}", path.display()))?;
    let config = toml::from_str::<RuleConfigFile>(&content)
        .map_err(|err| format!("failed to parse rule file '{}': {err}", path.display()))?;
    validate_rule_config(&config)?;
    Ok(config)
}

pub fn apply_threshold_overrides(thresholds: &mut ThresholdConfig, overrides: &RuleThresholdOverrides) {
    if let Some(value) = overrides.rom_usage_warn {
        thresholds.rom_percent = normalize_ratio_or_percent(value);
    }
    if let Some(value) = overrides.ram_usage_warn {
        thresholds.ram_percent = normalize_ratio_or_percent(value);
    }
    if let Some(value) = overrides.unknown_source_warn {
        thresholds.unknown_source_ratio = normalize_ratio_or_percent(value) / 100.0;
    }
    if let Some(value) = overrides.symbol_growth_warn_bytes {
        thresholds.symbol_growth_bytes = value;
    }
    if let Some(value) = overrides.large_symbol_warn_bytes {
        thresholds.large_symbol_bytes = value;
    }
    if let Some(value) = overrides.section_growth_warn_percent {
        thresholds.section_growth_rate = normalize_ratio_or_percent(value);
    }
    if let Some(value) = overrides.region_low_free_warn_bytes {
        thresholds.region_low_free_bytes = value;
    }
}

fn validate_rule_config(config: &RuleConfigFile) -> Result<(), String> {
    if config.schema_version != 1 {
        return Err(format!(
            "unsupported rule schema_version {}, expected 1",
            config.schema_version
        ));
    }
    for rule in &config.rules {
        validate_rule(rule)?;
    }
    Ok(())
}

fn validate_rule(rule: &CustomRule) -> Result<(), String> {
    if rule.id.trim().is_empty() {
        return Err("rule id must not be empty".to_string());
    }
    if rule.message.trim().is_empty() {
        return Err(format!("rule '{}' must have a non-empty message", rule.id));
    }
    match rule.kind {
        RuleKind::RegionUsage => {
            require_field(rule, rule.region.as_deref(), "region")?;
            require_float(rule, rule.warn_if_greater_than, "warn_if_greater_than")?;
        }
        RuleKind::SectionDelta => {
            require_field(rule, rule.section.as_deref(), "section")?;
            require_int(rule, rule.warn_if_delta_bytes_gt, "warn_if_delta_bytes_gt")?;
        }
        RuleKind::SymbolDelta => {
            require_field(rule, rule.symbol.as_deref(), "symbol")?;
            require_int(rule, rule.warn_if_delta_bytes_gt, "warn_if_delta_bytes_gt")?;
        }
        RuleKind::SymbolMatch => {
            require_field(rule, rule.symbol.as_deref(), "symbol")?;
        }
        RuleKind::ObjectMatch => {
            require_field(rule, rule.object.as_deref(), "object")?;
        }
        RuleKind::SourcePathGrowth => {
            require_field(rule, rule.pattern.as_deref(), "pattern")?;
            require_int(rule, rule.threshold_bytes.or(rule.warn_if_delta_bytes_gt), "threshold_bytes")?;
        }
        RuleKind::FunctionGrowth => {
            require_field(rule, rule.pattern.as_deref(), "pattern")?;
            require_int(rule, rule.threshold_bytes.or(rule.warn_if_delta_bytes_gt), "threshold_bytes")?;
        }
        RuleKind::UnknownSourceRatio => {
            require_float(rule, rule.warn_if_greater_than, "warn_if_greater_than")?;
        }
    }
    Ok(())
}

fn require_field(rule: &CustomRule, value: Option<&str>, field: &str) -> Result<(), String> {
    if value.is_none() {
        Err(format!("rule '{}' requires field '{}'", rule.id, field))
    } else {
        Ok(())
    }
}

fn require_float(rule: &CustomRule, value: Option<f64>, field: &str) -> Result<(), String> {
    if value.is_none() {
        Err(format!("rule '{}' requires field '{}'", rule.id, field))
    } else {
        Ok(())
    }
}

fn require_int(rule: &CustomRule, value: Option<i64>, field: &str) -> Result<(), String> {
    if value.is_none() {
        Err(format!("rule '{}' requires field '{}'", rule.id, field))
    } else {
        Ok(())
    }
}

fn normalize_ratio_or_percent(value: f64) -> f64 {
    if value <= 1.0 { value * 100.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::{apply_threshold_overrides, load_rule_config};
    use crate::model::ThresholdConfig;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reads_toml_rule_file_and_overrides_thresholds() {
        let path = std::env::temp_dir().join(format!(
            "fwmap-rules-{}.toml",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::write(
            &path,
            r#"
schema_version = 1

[thresholds]
rom_usage_warn = 0.9
symbol_growth_warn_bytes = 8192

[[rules]]
id = "flash-near-full"
kind = "region_usage"
region = "FLASH"
warn_if_greater_than = 0.92
severity = "warn"
message = "FLASH usage is above 92%"
"#,
        )
        .unwrap();
        let config = load_rule_config(&path).unwrap();
        let mut thresholds = ThresholdConfig::default();
        apply_threshold_overrides(&mut thresholds, &config.thresholds);
        assert_eq!(thresholds.rom_percent, 90.0);
        assert_eq!(thresholds.symbol_growth_bytes, 8192);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_invalid_rule_file() {
        let path = std::env::temp_dir().join(format!(
            "fwmap-rules-bad-{}.toml",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::write(
            &path,
            r#"
schema_version = 1

[[rules]]
id = "broken"
kind = "region_usage"
severity = "warn"
message = "oops"
"#,
        )
        .unwrap();
        let err = load_rule_config(&path).unwrap_err();
        assert!(err.contains("requires field 'region'"));
        let _ = fs::remove_file(path);
    }
}
