//! Configuration discovery and reload support.

use std::env;
use std::ffi::OsString;
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
    discover_config_from_env(
        env::var_os("NOVA_CONFIG"),
        env::var_os("XDG_CONFIG_HOME"),
        env::var_os("HOME"),
    )
}

fn discover_config_from_env(
    nova_config: Option<OsString>,
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    nova_config
        .map(PathBuf::from)
        .or_else(|| xdg_config_home.map(PathBuf::from).map(config_path_in))
        .or_else(|| {
            home.map(PathBuf::from)
                .map(|path| config_path_in(path.join(".config")))
        })
}

fn config_path_in(config_home: PathBuf) -> PathBuf {
    config_home.join("nova").join("config.toml")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::discover_config_from_env;

    #[test]
    fn discovers_explicit_nova_config_first() {
        assert_eq!(
            discover_config_from_env(
                Some(OsString::from("/tmp/custom.toml")),
                Some(OsString::from("/tmp/xdg")),
                Some(OsString::from("/tmp/home")),
            ),
            Some(PathBuf::from("/tmp/custom.toml"))
        );
    }

    #[test]
    fn discovers_xdg_config_home_config_toml() {
        assert_eq!(
            discover_config_from_env(
                None,
                Some(OsString::from("/tmp/xdg")),
                Some(OsString::from("/tmp/home")),
            ),
            Some(PathBuf::from("/tmp/xdg/nova/config.toml"))
        );
    }

    #[test]
    fn discovers_home_config_toml_without_xdg_config_home() {
        assert_eq!(
            discover_config_from_env(None, None, Some(OsString::from("/tmp/home"))),
            Some(PathBuf::from("/tmp/home/.config/nova/config.toml"))
        );
    }
}
