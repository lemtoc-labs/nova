//! User configuration model, loading, and validation.

pub mod error;
pub mod load;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use self::error::ConfigError;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    pub layout: LayoutConfig,
    pub segments: BTreeMap<String, SegmentConfig>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LayoutConfig {
    pub lines: u8,
    pub line1: LineConfig,
    pub line2: LineConfig,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LineConfig {
    pub left: Vec<String>,
    pub right: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SegmentConfig {
    pub max_components: Option<usize>,
    pub min_ms: Option<u64>,
    pub ttl_ms: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub style: StyleConfig,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct StyleConfig {
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub bold: bool,
}

impl Config {
    pub fn from_toml(input: &str) -> Result<Self, ConfigError> {
        let config = toml::from_str::<Self>(input).map_err(|source| ConfigError::Parse {
            path: Path::new("<inline>").to_path_buf(),
            source,
        })?;
        config.validate()
    }

    pub fn from_path(path: &Path) -> Result<Self, ConfigError> {
        let input = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let config = toml::from_str::<Self>(&input).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        config.validate()
    }

    pub fn from_optional_path(path: &Path) -> Result<Self, ConfigError> {
        if path.exists() {
            Self::from_path(path)
        } else {
            Ok(Self::default())
        }
    }

    pub fn segment(&self, id: &str) -> SegmentConfig {
        self.segments.get(id).cloned().unwrap_or_default()
    }

    fn validate(self) -> Result<Self, ConfigError> {
        match self.layout.lines {
            1 | 2 => Ok(self),
            lines => Err(ConfigError::InvalidLayoutLines { lines }),
        }
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            lines: 2,
            line1: LineConfig {
                left: vec![
                    "dir".to_string(),
                    "git_branch".to_string(),
                    "git_status".to_string(),
                    "rust_version".to_string(),
                ],
                right: vec!["duration".to_string()],
            },
            line2: LineConfig {
                left: vec!["exit_status".to_string(), "prompt_char".to_string()],
                right: Vec::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_a_two_line_sync_layout() {
        let config = Config::default();

        assert_eq!(config.layout.lines, 2);
        assert_eq!(
            config.layout.line1.left,
            ["dir", "git_branch", "git_status", "rust_version"]
        );
        assert_eq!(config.layout.line1.right, ["duration"]);
        assert_eq!(config.layout.line2.left, ["exit_status", "prompt_char"]);
    }

    #[test]
    fn parses_segment_settings() {
        let config = Config::from_toml(
            r#"
            [layout]
            lines = 1

            [layout.line1]
            left = ["dir", "prompt_char"]
            right = []

            [segments.dir]
            max_components = 2
            ttl_ms = 5000
            timeout_ms = 1234
            style = { fg = "blue", bold = true }
            "#,
        )
        .expect("config should parse");

        let dir = config.segment("dir");
        assert_eq!(config.layout.lines, 1);
        assert_eq!(dir.max_components, Some(2));
        assert_eq!(dir.ttl_ms, Some(5_000));
        assert_eq!(dir.timeout_ms, Some(1_234));
        assert_eq!(dir.style.fg.as_deref(), Some("blue"));
        assert!(dir.style.bold);
    }

    #[test]
    fn rejects_unsupported_layout_line_counts() {
        let error =
            Config::from_toml("[layout]\nlines = 3\n").expect_err("invalid layout should fail");

        assert!(matches!(
            error,
            ConfigError::InvalidLayoutLines { lines: 3 }
        ));
    }
}
