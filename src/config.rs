use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt;

/// Mode for handling unsupported import patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnsupportedImportMode {
    /// Throw an error and stop compilation
    #[default]
    Error,
    /// Print a warning and skip the import
    Warn,
    /// Silently skip the import
    Off,
}

impl fmt::Display for UnsupportedImportMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnsupportedImportMode::Error => write!(f, "error"),
            UnsupportedImportMode::Warn => write!(f, "warn"),
            UnsupportedImportMode::Off => write!(f, "off"),
        }
    }
}

impl<'de> Deserialize<'de> for UnsupportedImportMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "error" => Ok(UnsupportedImportMode::Error),
            "warn" => Ok(UnsupportedImportMode::Warn),
            "off" => Ok(UnsupportedImportMode::Off),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid unsupported_import_mode '{}'. Valid options are: error, warn, off",
                s
            ))),
        }
    }
}

/// Mode for handling invalid barrel files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InvalidBarrelMode {
    /// Throw an error and stop compilation
    #[default]
    Error,
    /// Print a warning and skip the import
    Warn,
    /// Silently skip the import
    Off,
}

impl fmt::Display for InvalidBarrelMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvalidBarrelMode::Error => write!(f, "error"),
            InvalidBarrelMode::Warn => write!(f, "warn"),
            InvalidBarrelMode::Off => write!(f, "off"),
        }
    }
}

impl<'de> Deserialize<'de> for InvalidBarrelMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "error" => Ok(InvalidBarrelMode::Error),
            "warn" => Ok(InvalidBarrelMode::Warn),
            "off" => Ok(InvalidBarrelMode::Off),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid invalid_barrel_mode '{}'. Valid options are: error, warn, off",
                s
            ))),
        }
    }
}

/// Configuration for the barrel files plugin
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Patterns for barrel files
    pub patterns: Vec<String>,

    /// Rules for resolving import aliases (optional)
    pub aliases: Option<Vec<Alias>>,

    /// Symlink mappings from external paths to internal paths (optional)
    pub symlinks: Option<HashMap<String, String>>,

    /// Enables debug logging to stdout
    pub debug: Option<bool>,

    /// How to handle unsupported import patterns (e.g. namespace imports)
    #[serde(default)]
    pub unsupported_import_mode: UnsupportedImportMode,

    /// How to handle invalid barrel files (files with unsupported constructs)
    #[serde(default)]
    pub invalid_barrel_mode: InvalidBarrelMode,
}

/// Rule for resolving import aliases
#[derive(Debug, Deserialize, Clone)]
pub struct Alias {
    /// Pattern to match against import paths.
    pub pattern: String,
    /// Paths to resolve the matched imports to.
    pub paths: Vec<String>,
    /// Directories for which the alias should be applied (optional).
    pub context: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parsing() {
        let config_json = r#"{
            "aliases": [
                {
                    "pattern": "@features/*",
                    "paths": ["src/features/*/index.ts"],
                    "context": ["src"]
                }
            ],
            "patterns": [
                "src/entities/*/index.ts",
                "src/features/*/index.ts"
            ],
            "cache_duration_ms": 1000
        }"#;

        let config: Config =
            serde_json::from_str(config_json).expect("Failed to parse config JSON");

        assert!(config.aliases.is_some());
        let aliases = config.aliases.unwrap();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].pattern, "@features/*");
        assert_eq!(&aliases[0].paths, &vec!["src/features/*/index.ts"]);
        let context = aliases[0].context.as_ref().unwrap();
        assert_eq!(context, &vec!["src"]);
        let patterns = config.patterns;
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0], "src/entities/*/index.ts");
        assert_eq!(patterns[1], "src/features/*/index.ts");
    }

    #[test]
    fn test_mode_validation() {
        let config_json = r#"{
            "patterns": ["src/*/index.ts"],
            "unsupported_import_mode": "warn",
            "invalid_barrel_mode": "off"
        }"#;

        let config: Config =
            serde_json::from_str(config_json).expect("Failed to parse config JSON");

        assert_eq!(config.unsupported_import_mode, UnsupportedImportMode::Warn);
        assert_eq!(config.invalid_barrel_mode, InvalidBarrelMode::Off);
    }

    #[test]
    fn test_mode_defaults() {
        let config_json = r#"{
            "patterns": ["src/*/index.ts"]
        }"#;

        let config: Config =
            serde_json::from_str(config_json).expect("Failed to parse config JSON");

        assert_eq!(config.unsupported_import_mode, UnsupportedImportMode::Error);
        assert_eq!(config.invalid_barrel_mode, InvalidBarrelMode::Error);
    }

    #[test]
    fn test_invalid_mode_validation() {
        let config_json = r#"{
            "patterns": ["src/*/index.ts"],
            "unsupported_import_mode": "invalid",
            "invalid_barrel_mode": "invalid"
        }"#;

        let result: Result<Config, _> = serde_json::from_str(config_json);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Invalid unsupported_import_mode"));
    }

    #[test]
    fn test_all_valid_modes() {
        let test_cases = vec![
            (
                "error",
                UnsupportedImportMode::Error,
                InvalidBarrelMode::Error,
            ),
            ("warn", UnsupportedImportMode::Warn, InvalidBarrelMode::Warn),
            ("off", UnsupportedImportMode::Off, InvalidBarrelMode::Off),
        ];

        for (mode_str, expected_unsupported, expected_invalid) in test_cases {
            let config_json = format!(
                r#"{{
                    "patterns": ["src/*/index.ts"],
                    "unsupported_import_mode": "{}",
                    "invalid_barrel_mode": "{}"
                }}"#,
                mode_str, mode_str
            );

            let config: Config =
                serde_json::from_str(&config_json).expect("Failed to parse config JSON");

            assert_eq!(config.unsupported_import_mode, expected_unsupported);
            assert_eq!(config.invalid_barrel_mode, expected_invalid);
        }
    }

    #[test]
    fn test_enum_display() {
        assert_eq!(UnsupportedImportMode::Error.to_string(), "error");
        assert_eq!(UnsupportedImportMode::Warn.to_string(), "warn");
        assert_eq!(UnsupportedImportMode::Off.to_string(), "off");

        assert_eq!(InvalidBarrelMode::Error.to_string(), "error");
        assert_eq!(InvalidBarrelMode::Warn.to_string(), "warn");
        assert_eq!(InvalidBarrelMode::Off.to_string(), "off");
    }
}
