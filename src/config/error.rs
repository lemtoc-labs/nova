//! Configuration diagnostics.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config `{path}`: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid config: layout.lines must be 1 or 2, got {lines}")]
    InvalidLayoutLines { lines: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigWarning {
    UnknownKey { location: String },
    UnknownSegmentTable { segment: String },
    UnknownLayoutSegment { location: String, segment: String },
    InvalidColor { location: String, color: String },
}

impl std::fmt::Display for ConfigWarning {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownKey { location } => {
                write!(formatter, "unknown config key `{location}`")
            }
            Self::UnknownSegmentTable { segment } => {
                write!(formatter, "unknown segment table `segments.{segment}`")
            }
            Self::UnknownLayoutSegment { location, segment } => {
                write!(formatter, "unknown segment `{segment}` in `{location}`")
            }
            Self::InvalidColor { location, color } => {
                write!(formatter, "invalid color `{color}` in `{location}`")
            }
        }
    }
}
