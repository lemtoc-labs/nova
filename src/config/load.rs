//! Configuration discovery and reload support.

use std::env;
use std::path::{Path, PathBuf};

use super::Config;
use super::error::ConfigError;

pub fn load_config(path: Option<&Path>) -> Result<Config, ConfigError> {
    match path {
        Some(path) => Config::from_path(path),
        None => discover_config().map_or_else(
            || Ok(Config::default()),
            |path| Config::from_optional_path(&path),
        ),
    }
}

fn discover_config() -> Option<PathBuf> {
    env::var_os("NOVA_CONFIG")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .map(|path| path.join("nova").join("nova.toml"))
        })
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|path| path.join(".config").join("nova").join("nova.toml"))
        })
}
