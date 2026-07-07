//! Configuration discovery and reload support.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::Config;
use super::error::ConfigError;

pub fn load_config(path: Option<&Path>) -> Result<Config, ConfigError> {
    match path {
        Some(path) => Config::from_path(path),
        None => ConfigSource::discover().load().map(|loaded| loaded.config),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigSource {
    path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigFingerprint {
    modified: SystemTime,
    len: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigSnapshot {
    pub path: Option<PathBuf>,
    pub fingerprint: Option<ConfigFingerprint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedConfig {
    pub snapshot: ConfigSnapshot,
    pub config: Config,
}

impl ConfigSource {
    pub fn discover() -> Self {
        Self {
            path: discover_config(),
        }
    }

    #[cfg(test)]
    pub fn from_path(path: Option<PathBuf>) -> Self {
        Self { path }
    }

    pub fn snapshot(&self) -> Result<ConfigSnapshot, ConfigError> {
        let Some(path) = &self.path else {
            return Ok(ConfigSnapshot {
                path: None,
                fingerprint: None,
            });
        };

        Ok(ConfigSnapshot {
            path: Some(path.clone()),
            fingerprint: config_fingerprint(path)?,
        })
    }

    pub fn load(&self) -> Result<LoadedConfig, ConfigError> {
        let snapshot = self.snapshot()?;
        let config = self.load_snapshot(&snapshot)?;
        Ok(LoadedConfig { snapshot, config })
    }

    pub fn load_snapshot(&self, snapshot: &ConfigSnapshot) -> Result<Config, ConfigError> {
        match (&snapshot.path, snapshot.fingerprint) {
            (Some(path), Some(_fingerprint)) => Config::from_path(path),
            (Some(_), None) | (None, None) => Ok(Config::default()),
            (None, Some(_fingerprint)) => Ok(Config::default()),
        }
    }
}

fn config_fingerprint(path: &Path) -> Result<Option<ConfigFingerprint>, ConfigError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let modified = metadata.modified().map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(Some(ConfigFingerprint {
        modified,
        len: metadata.len(),
    }))
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
    use std::fs;
    use std::path::PathBuf;

    use super::{ConfigSource, discover_config_from_env};

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

    #[test]
    fn snapshots_missing_config_path_without_fingerprint() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let path = tempdir.path().join("missing.toml");
        let source = ConfigSource::from_path(Some(path.clone()));

        let snapshot = source.snapshot().expect("snapshot should succeed");

        assert_eq!(snapshot.path.as_deref(), Some(path.as_path()));
        assert_eq!(snapshot.fingerprint, None);
    }

    #[test]
    fn snapshots_existing_config_path_with_fingerprint() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let path = tempdir.path().join("config.toml");
        fs::write(&path, "[layout]\nlines = 1\n").expect("config should be written");
        let source = ConfigSource::from_path(Some(path.clone()));

        let snapshot = source.snapshot().expect("snapshot should succeed");

        assert_eq!(snapshot.path.as_deref(), Some(path.as_path()));
        assert!(snapshot.fingerprint.is_some());
    }
}
