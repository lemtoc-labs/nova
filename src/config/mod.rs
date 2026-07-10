//! User configuration model, loading, and validation.

pub mod error;
pub mod load;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use serde::Deserialize;

use self::error::{ConfigError, ConfigWarning};

pub const DEFAULT_INITIAL_WAIT_MS: u64 = 0;
pub const DEFAULT_MIN_LOADING_MS: u64 = 50;
static DEFAULT_SEGMENT_CONFIG: LazyLock<SegmentConfig> = LazyLock::new(SegmentConfig::default);

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(rename = "async")]
    pub async_config: AsyncConfig,
    pub layout: LayoutConfig,
    pub segments: BTreeMap<String, SegmentConfig>,
    #[serde(skip)]
    pub(crate) unknown_keys: BTreeSet<String>,
}

impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        self.async_config == other.async_config
            && self.layout == other.layout
            && self.segments == other.segments
    }
}

impl Eq for Config {}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AsyncConfig {
    pub initial_wait_ms: Option<u64>,
    pub min_loading_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LayoutConfig {
    pub lines: u8,
    pub separator: Option<String>,
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
    pub character: Option<String>,
    pub characters: BTreeMap<String, String>,
    pub icon: Option<String>,
    pub icons: BTreeMap<String, String>,
    pub loading: Option<String>,
    pub max_components: Option<usize>,
    pub min_ms: Option<u64>,
    pub min_loading_ms: Option<u64>,
    pub force_display: Option<bool>,
    pub format: Option<String>,
    pub prefix: Option<String>,
    pub separator: Option<String>,
    pub show_counts: Option<bool>,
    pub ttl_ms: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub style: StyleConfig,
    pub error_style: StyleConfig,
    pub prefix_style: StyleConfig,
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
        Self::parse(input, Path::new("<inline>"))
    }

    pub fn from_path(path: &Path) -> Result<Self, ConfigError> {
        let input = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&input, path)
    }

    fn parse(input: &str, path: &Path) -> Result<Self, ConfigError> {
        let mut unknown_keys = BTreeSet::new();
        let deserializer = toml::Deserializer::new(input);
        let mut config: Self = serde_ignored::deserialize(deserializer, |unknown| {
            unknown_keys.insert(unknown.to_string());
        })
        .map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        config.unknown_keys = unknown_keys;
        config.validate()
    }

    pub fn from_optional_path(path: &Path) -> Result<Self, ConfigError> {
        if path.exists() {
            Self::from_path(path)
        } else {
            Ok(Self::default())
        }
    }

    pub fn segment(&self, id: &str) -> &SegmentConfig {
        self.segments.get(id).unwrap_or(&DEFAULT_SEGMENT_CONFIG)
    }

    pub fn warnings(&self, known_segment_ids: &[&str]) -> Vec<ConfigWarning> {
        let mut warnings = self
            .unknown_keys
            .iter()
            .map(|location| ConfigWarning::UnknownKey {
                location: location.clone(),
            })
            .collect::<Vec<_>>();
        let mut seen = BTreeSet::new();

        for (location, segment) in self.layout_segments() {
            if known_segment_ids.contains(&segment) {
                continue;
            }

            if seen.insert((location, segment)) {
                warnings.push(ConfigWarning::UnknownLayoutSegment {
                    location: location.to_string(),
                    segment: segment.to_string(),
                });
            }
        }

        for segment_id in self.segments.keys() {
            if !known_segment_ids.contains(&segment_id.as_str()) {
                warnings.push(ConfigWarning::UnknownSegmentTable {
                    segment: segment_id.clone(),
                });
            }
        }

        for (segment_id, segment_config) in &self.segments {
            push_style_warnings(
                &mut warnings,
                &format!("segments.{segment_id}.style"),
                &segment_config.style,
            );
            push_style_warnings(
                &mut warnings,
                &format!("segments.{segment_id}.error_style"),
                &segment_config.error_style,
            );
            push_style_warnings(
                &mut warnings,
                &format!("segments.{segment_id}.prefix_style"),
                &segment_config.prefix_style,
            );
        }

        warnings
    }

    fn validate(self) -> Result<Self, ConfigError> {
        match self.layout.lines {
            1 | 2 => Ok(self),
            lines => Err(ConfigError::InvalidLayoutLines { lines }),
        }
    }

    fn layout_segments(&self) -> impl Iterator<Item = (&'static str, &str)> {
        [
            ("layout.line1.left", self.layout.line1.left.as_slice()),
            ("layout.line1.right", self.layout.line1.right.as_slice()),
            ("layout.line2.left", self.layout.line2.left.as_slice()),
            ("layout.line2.right", self.layout.line2.right.as_slice()),
        ]
        .into_iter()
        .flat_map(|(location, segments)| {
            segments
                .iter()
                .map(move |segment| (location, segment.as_str()))
        })
    }
}

fn push_style_warnings(warnings: &mut Vec<ConfigWarning>, location: &str, style: &StyleConfig) {
    if let Some(color) = &style.fg
        && !is_supported_color(color)
    {
        warnings.push(ConfigWarning::InvalidColor {
            location: format!("{location}.fg"),
            color: color.clone(),
        });
    }

    if let Some(color) = &style.bg
        && !is_supported_color(color)
    {
        warnings.push(ConfigWarning::InvalidColor {
            location: format!("{location}.bg"),
            color: color.clone(),
        });
    }
}

fn is_supported_color(color: &str) -> bool {
    is_named_color(color) || color.parse::<u8>().is_ok() || is_truecolor(color)
}

fn is_named_color(color: &str) -> bool {
    matches!(
        color,
        "black"
            | "red"
            | "green"
            | "yellow"
            | "blue"
            | "magenta"
            | "cyan"
            | "white"
            | "bright_black"
            | "bright_red"
            | "bright_green"
            | "bright_yellow"
            | "bright_blue"
            | "bright_magenta"
            | "bright_cyan"
            | "bright_white"
    )
}

fn is_truecolor(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            lines: 2,
            separator: None,
            line1: LineConfig {
                left: vec![
                    "ssh".to_string(),
                    "dir".to_string(),
                    "git_branch".to_string(),
                    "git_status".to_string(),
                    "rust_version".to_string(),
                    "bun_version".to_string(),
                    "deno_version".to_string(),
                    "node_version".to_string(),
                    "python_version".to_string(),
                    "nix_shell".to_string(),
                    "aws".to_string(),
                    "duration".to_string(),
                ],
                right: vec!["time".to_string()],
            },
            line2: LineConfig {
                left: vec!["exit_status".to_string(), "prompt_char".to_string()],
                right: Vec::new(),
            },
        }
    }
}

impl LayoutConfig {
    pub fn separator(&self) -> &str {
        self.separator.as_deref().unwrap_or(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KNOWN_SEGMENTS: &[&str] = &[
        "aws",
        "bun_version",
        "deno_version",
        "dir",
        "duration",
        "exit_status",
        "git_branch",
        "git_status",
        "nix_shell",
        "node_version",
        "prompt_char",
        "python_version",
        "rust_version",
        "ssh",
        "time",
        "user_host",
    ];

    #[test]
    fn defaults_to_a_two_line_sync_layout() {
        let config = Config::default();

        assert_eq!(config.async_config.initial_wait_ms, None);
        assert_eq!(config.async_config.min_loading_ms, None);
        assert_eq!(config.layout.lines, 2);
        assert_eq!(config.layout.separator, None);
        assert_eq!(config.layout.separator(), " ");
        assert_eq!(
            config.layout.line1.left,
            [
                "ssh",
                "dir",
                "git_branch",
                "git_status",
                "rust_version",
                "bun_version",
                "deno_version",
                "node_version",
                "python_version",
                "nix_shell",
                "aws",
                "duration"
            ]
        );
        assert_eq!(config.layout.line1.right, ["time"]);
        assert_eq!(config.layout.line2.left, ["exit_status", "prompt_char"]);
    }

    #[test]
    fn parses_segment_settings() {
        let config = Config::from_toml(
            r##"
            [layout]
            lines = 1
            separator = " | "

            [async]
            initial_wait_ms = 10
            min_loading_ms = 50

            [layout.line1]
            left = ["dir", "prompt_char"]
            right = []

            [segments.dir]
            icon = "d"
            max_components = 2
            ttl_ms = 5000
            timeout_ms = 1234
            style = { fg = "blue", bold = true }

            [segments.prompt_char]
            character = ">_"
            characters = { vi_command = "%" }
            error_style = { fg = "red", bold = true }

            [segments.duration]
            prefix = "took "
            prefix_style = { fg = "#33ccff", bold = true }

            [segments.aws]
            force_display = false
            format = "$symbol$profile"

            [segments.git_status]
            loading = "…"
            min_loading_ms = 25
            separator = " "
            show_counts = true
            icons = { staged = "S", untracked = "U", stash = "T" }
            style = { fg = "202", bg = "#102030" }
            "##,
        )
        .expect("config should parse");

        assert_eq!(config.async_config.initial_wait_ms, Some(10));
        assert_eq!(config.async_config.min_loading_ms, Some(50));
        let dir = config.segment("dir");
        let prompt_char = config.segment("prompt_char");
        let duration = config.segment("duration");
        let aws = config.segment("aws");
        let git_status = config.segment("git_status");
        assert_eq!(config.layout.lines, 1);
        assert_eq!(config.layout.separator.as_deref(), Some(" | "));
        assert_eq!(config.layout.separator(), " | ");
        assert_eq!(dir.icon.as_deref(), Some("d"));
        assert_eq!(dir.max_components, Some(2));
        assert_eq!(dir.ttl_ms, Some(5_000));
        assert_eq!(dir.timeout_ms, Some(1_234));
        assert_eq!(dir.style.fg.as_deref(), Some("blue"));
        assert!(dir.style.bold);
        assert_eq!(prompt_char.character.as_deref(), Some(">_"));
        assert_eq!(
            prompt_char.characters.get("vi_command").map(String::as_str),
            Some("%")
        );
        assert_eq!(prompt_char.error_style.fg.as_deref(), Some("red"));
        assert!(prompt_char.error_style.bold);
        assert_eq!(aws.force_display, Some(false));
        assert_eq!(aws.format.as_deref(), Some("$symbol$profile"));
        assert_eq!(duration.prefix.as_deref(), Some("took "));
        assert_eq!(duration.prefix_style.fg.as_deref(), Some("#33ccff"));
        assert!(duration.prefix_style.bold);
        assert_eq!(git_status.style.fg.as_deref(), Some("202"));
        assert_eq!(git_status.style.bg.as_deref(), Some("#102030"));
        assert_eq!(git_status.loading.as_deref(), Some("…"));
        assert_eq!(git_status.min_loading_ms, Some(25));
        assert_eq!(git_status.separator.as_deref(), Some(" "));
        assert_eq!(git_status.show_counts, Some(true));
        assert_eq!(
            git_status.icons.get("staged").map(String::as_str),
            Some("S")
        );
        assert_eq!(
            git_status.icons.get("untracked").map(String::as_str),
            Some("U")
        );
        assert_eq!(git_status.icons.get("stash").map(String::as_str), Some("T"));
    }

    #[test]
    fn parses_example_config() {
        let config = Config::from_toml(include_str!("../../examples/config.toml"))
            .expect("example config should parse");

        assert_eq!(config.warnings(TEST_KNOWN_SEGMENTS), []);
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

    #[test]
    fn preserves_source_spans_for_invalid_config_values() {
        let error = Config::from_toml("[layout]\nlines = \"two\"\n")
            .expect_err("invalid value should fail");
        let ConfigError::Parse { source, .. } = error else {
            panic!("invalid value should produce a parse error");
        };

        assert!(source.span().is_some());
    }

    #[test]
    fn warns_about_unknown_config_keys_and_segment_tables() {
        let config = Config::from_toml(
            r#"
            unexpected = true

            [async]
            initial_wait = 10

            [layout]
            lines = 1
            seperator = " "

            [layout.line1]
            left = ["dir"]
            right = []
            middle = []

            [segments.git_status]
            show_count = true
            icons = { custom = "x" }
            style = { fg = "red", foreground = "blue" }

            [segments.git_staus]
            style = { fg = "red" }

            [segments.time]
            max_components = 2
            "#,
        )
        .expect("config should parse");

        assert_eq!(
            config.warnings(TEST_KNOWN_SEGMENTS),
            [
                ConfigWarning::UnknownKey {
                    location: "async.initial_wait".to_string(),
                },
                ConfigWarning::UnknownKey {
                    location: "layout.line1.middle".to_string(),
                },
                ConfigWarning::UnknownKey {
                    location: "layout.seperator".to_string(),
                },
                ConfigWarning::UnknownKey {
                    location: "segments.git_status.show_count".to_string(),
                },
                ConfigWarning::UnknownKey {
                    location: "segments.git_status.style.foreground".to_string(),
                },
                ConfigWarning::UnknownKey {
                    location: "unexpected".to_string(),
                },
                ConfigWarning::UnknownSegmentTable {
                    segment: "git_staus".to_string(),
                },
            ]
        );
        assert_eq!(
            Config::from_toml("unexpected = true").expect("unknown key should remain valid"),
            Config::default()
        );
    }

    #[test]
    fn warns_about_unknown_layout_segments() {
        let config = Config::from_toml(
            r#"
            [layout]
            lines = 2

            [layout.line1]
            left = ["dir", "missing"]
            right = ["missing"]

            [layout.line2]
            left = ["prompt_char"]
            right = []
            "#,
        )
        .expect("config should parse");

        assert_eq!(
            config.warnings(TEST_KNOWN_SEGMENTS),
            [
                ConfigWarning::UnknownLayoutSegment {
                    location: "layout.line1.left".to_string(),
                    segment: "missing".to_string(),
                },
                ConfigWarning::UnknownLayoutSegment {
                    location: "layout.line1.right".to_string(),
                    segment: "missing".to_string(),
                }
            ]
        );
    }

    #[test]
    fn warns_about_invalid_colors() {
        let config = Config::from_toml(
            r##"
            [layout]
            lines = 1

            [layout.line1]
            left = ["dir"]
            right = []

            [segments.dir]
            style = { fg = "not-a-color", bg = "#12345g" }
            error_style = { fg = "256" }
            prefix_style = { bg = "#123456" }
            "##,
        )
        .expect("config should parse");

        assert_eq!(
            config.warnings(TEST_KNOWN_SEGMENTS),
            [
                ConfigWarning::InvalidColor {
                    location: "segments.dir.style.fg".to_string(),
                    color: "not-a-color".to_string(),
                },
                ConfigWarning::InvalidColor {
                    location: "segments.dir.style.bg".to_string(),
                    color: "#12345g".to_string(),
                },
                ConfigWarning::InvalidColor {
                    location: "segments.dir.error_style.fg".to_string(),
                    color: "256".to_string(),
                }
            ]
        );
    }
}
