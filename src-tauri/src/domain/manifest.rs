use serde::Deserialize;

use crate::error::AppError;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ManifestSummary {
    pub version: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub icon: Option<String>,
    pub depends: Vec<String>,
    pub bin: Vec<String>,
    pub persist: Vec<String>,
    pub deprecated: bool,
}

#[derive(Deserialize)]
struct Raw {
    version: Option<String>,
    description: Option<String>,
    homepage: Option<String>,
    license: Option<serde_json::Value>,
    icon: Option<String>,
    depends: Option<serde_json::Value>,
    bin: Option<serde_json::Value>,
    persist: Option<serde_json::Value>,
    deprecated: Option<bool>,
}

fn strings_from(v: &serde_json::Value) -> Vec<String> {
    match v {
        serde_json::Value::String(s) => vec![s.clone()],
        serde_json::Value::Array(items) => items
            .iter()
            .flat_map(|item| match item {
                serde_json::Value::String(s) => vec![s.clone()],
                serde_json::Value::Array(inner) => inner
                    .first()
                    .and_then(|x| x.as_str())
                    .map(|s| vec![s.to_string()])
                    .unwrap_or_default(),
                _ => Vec::new(),
            })
            .collect(),
        serde_json::Value::Object(map) => map.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

fn license_from(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(map) => map
            .get("identifier")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

pub fn parse(text: &str) -> Result<ManifestSummary, AppError> {
    let raw: Raw = serde_json::from_str(text)?;
    let version = raw
        .version
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Other("清单缺少 version 字段".into()))?;
    Ok(ManifestSummary {
        version,
        description: raw.description,
        homepage: raw.homepage,
        license: raw.license.as_ref().and_then(license_from),
        icon: raw.icon,
        depends: raw.depends.as_ref().map(strings_from).unwrap_or_default(),
        bin: raw.bin.as_ref().map(strings_from).unwrap_or_default(),
        persist: raw.persist.as_ref().map(strings_from).unwrap_or_default(),
        deprecated: raw.deprecated.unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_manifest() {
        let text = r#"{
            "version": "2.55.0.5",
            "description": "A free and open source distributed version control system.",
            "homepage": "https://gitforwindows.org",
            "license": { "identifier": "GPL-2.0-only", "url": "https://example.com" },
            "depends": ["less"],
            "bin": [["git", "git.exe"], "scalar.exe"],
            "persist": [["etc/gitconfig", "config"], "ssh"]
        }"#;
        let m = parse(text).unwrap();
        assert_eq!(m.version, "2.55.0.5");
        assert_eq!(m.license.as_deref(), Some("GPL-2.0-only"));
        assert_eq!(m.homepage.as_deref(), Some("https://gitforwindows.org"));
        assert_eq!(m.depends, vec!["less"]);
        assert_eq!(m.bin, vec!["git", "scalar.exe"]);
        assert_eq!(m.persist, vec!["etc/gitconfig", "ssh"]);
    }

    #[test]
    fn normalizes_string_and_object_forms() {
        let m = parse(r#"{"version":"1.0","license":"MIT","persist":{"data":"d"},"bin":"tool.exe"}"#).unwrap();
        assert_eq!(m.license.as_deref(), Some("MIT"));
        assert_eq!(m.persist, vec!["data"]);
        assert_eq!(m.bin, vec!["tool.exe"]);
    }

    #[test]
    fn rejects_manifest_without_version() {
        let err = parse(r#"{"description":"x"}"#).unwrap_err();
        assert!(err.to_string().contains("version"));
    }

    #[test]
    fn reads_deprecated_flag() {
        let m = parse(r#"{"version":"1.0","deprecated":true}"#).unwrap();
        assert!(m.deprecated);
        let m2 = parse(r#"{"version":"1.0"}"#).unwrap();
        assert!(!m2.deprecated);
    }
}
