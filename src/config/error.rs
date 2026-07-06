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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigWarning {
    UnknownLayoutSegment { location: String, segment: String },
}

impl std::fmt::Display for ConfigWarning {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownLayoutSegment { location, segment } => {
                write!(formatter, "unknown segment `{segment}` in `{location}`")
            }
        }
    }
}
