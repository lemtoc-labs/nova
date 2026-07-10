use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::cache::CacheKey;
use crate::config::SegmentConfig;
use crate::segments::{
    AsyncJobSegment, AsyncSegmentSpec, CollectContext, SegmentContent, Style, label_with_icon,
};
use crate::state::PromptState;

use super::{
    RUNTIME_REFRESH_TTL, RUNTIME_TIMEOUT, RuntimeCollectError, collect_optional_command_output,
    collected_version_segment, remaining_time, runtime_cache_source,
};

const RUST_VERSION_SEGMENT_ID: &str = "rust_version";
const RUST_VERSION_ICON: &str = "";
const RUSTC_ARGS: &[&str] = &["--version"];
const RUST_MARKERS: &[&str] = &["Cargo.toml", "rust-toolchain", "rust-toolchain.toml"];

pub struct RustSegment;

impl AsyncSegmentSpec for RustSegment {
    fn render_ids(&self) -> &'static [&'static str] {
        &[RUST_VERSION_SEGMENT_ID]
    }

    fn primary_id(&self) -> &'static str {
        RUST_VERSION_SEGMENT_ID
    }

    fn cache_key(
        &self,
        render_id: &str,
        state: &PromptState,
        config_generation: u64,
    ) -> Option<CacheKey> {
        (render_id == self.primary_id())
            .then(|| rust_cache_key(&state.cwd, state.env.path.as_deref(), config_generation))
            .flatten()
    }

    fn collect(&self, ctx: &CollectContext<'_>) -> Vec<AsyncJobSegment> {
        let Some(key) = self.cache_key(self.primary_id(), ctx.state, ctx.config_generation) else {
            return Vec::new();
        };
        let config = ctx.config.segment(self.primary_id());
        collected_version_segment(
            key,
            collect_rust_version(&ctx.state.cwd, ctx.state.env.path.as_deref(), ctx.deadline),
            |version| render_rust_version(version, config),
        )
    }

    fn default_ttl(&self) -> Duration {
        RUNTIME_REFRESH_TTL
    }

    fn default_timeout(&self) -> Duration {
        RUNTIME_TIMEOUT
    }
}

pub fn rust_cache_key(cwd: &Path, path: Option<&str>, config_generation: u64) -> Option<CacheKey> {
    let root = find_rust_project_root(cwd)?;
    Some(CacheKey::new(
        RUST_VERSION_SEGMENT_ID,
        runtime_cache_source(root.to_string_lossy(), path),
        config_generation,
    ))
}

pub fn find_rust_project_root(cwd: &Path) -> Option<PathBuf> {
    let mut current = cwd;

    loop {
        if RUST_MARKERS
            .iter()
            .any(|marker| current.join(marker).exists())
        {
            return Some(current.to_path_buf());
        }

        current = current.parent()?;
    }
}

pub fn collect_rust_version(
    cwd: &Path,
    path: Option<&str>,
    deadline: Instant,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_rust_version_with_command_and_path(cwd, deadline, Path::new("rustc"), path)
}

#[cfg(test)]
fn collect_rust_version_with_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_rust_version_with_command_and_path(cwd, deadline, command, None)
}

fn collect_rust_version_with_command_and_path(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
    path: Option<&str>,
) -> Result<Option<String>, RuntimeCollectError> {
    let Some(root) = find_rust_project_root(cwd) else {
        return Ok(None);
    };
    let timeout = remaining_time(deadline)?;
    let Some(output) = collect_optional_command_output(command, RUSTC_ARGS, &root, timeout, path)?
    else {
        return Ok(None);
    };

    Ok(parse_rustc_version(&String::from_utf8_lossy(&output)))
}

pub fn parse_rustc_version(output: &str) -> Option<String> {
    let mut parts = output.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some("rustc"), Some(version)) if !version.is_empty() => Some(version.to_string()),
        _ => None,
    }
}

pub fn render_rust_version(version: &str, config: &SegmentConfig) -> Option<SegmentContent> {
    if version.is_empty() {
        return None;
    }

    Some(SegmentContent::new(
        RUST_VERSION_SEGMENT_ID,
        label_with_icon(version, config, RUST_VERSION_ICON),
        rust_style(config),
    ))
}

fn rust_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("cyan".to_string()),
            bg: None,
            bold: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[cfg(unix)]
    use super::super::write_script;
    use super::*;

    #[test]
    fn finds_rust_project_root_from_markers() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let nested = tempdir.path().join("crates").join("nova");
        fs::create_dir_all(&nested).expect("nested dir should be created");
        fs::write(
            tempdir.path().join("Cargo.toml"),
            "[package]\nname = \"nova\"\n",
        )
        .expect("marker should be written");

        assert_eq!(
            find_rust_project_root(&nested),
            Some(tempdir.path().to_path_buf())
        );
    }

    #[test]
    fn cache_key_is_none_outside_rust_projects() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");

        assert_eq!(rust_cache_key(tempdir.path(), None, 1), None);
    }

    #[test]
    fn parses_rustc_version_output() {
        assert_eq!(
            parse_rustc_version("rustc 1.96.1 (abc 2026-01-01)\n"),
            Some("1.96.1".to_string())
        );
        assert_eq!(parse_rustc_version("not rustc\n"), None);
    }

    #[test]
    fn renders_rust_version_segment() {
        let segment = render_rust_version("1.96.1", &SegmentConfig::default())
            .expect("version should render");

        assert_eq!(segment.id, "rust_version");
        assert_eq!(segment.text, " 1.96.1");
        assert_eq!(segment.style.fg.as_deref(), Some("cyan"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_rust_version_with_configured_icon() {
        let segment = render_rust_version(
            "1.96.1",
            &SegmentConfig {
                icon: Some("rust".to_string()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "rust 1.96.1");
    }

    #[test]
    fn renders_rust_version_without_icon_when_configured_empty() {
        let segment = render_rust_version(
            "1.96.1",
            &SegmentConfig {
                icon: Some(String::new()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "1.96.1");
    }

    #[test]
    #[cfg(unix)]
    fn collects_rust_version_with_timeout_bound_command() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(
            tempdir.path().join("Cargo.toml"),
            "[package]\nname = \"nova\"\n",
        )
        .expect("marker should be written");
        let rustc = write_script(
            tempdir.path(),
            "rustc",
            "printf 'rustc 1.96.1 (abc date)\\n'\n",
        );

        let collected = collect_rust_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_secs(5),
            &rustc,
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("1.96.1".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn times_out_slow_rust_version_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(
            tempdir.path().join("Cargo.toml"),
            "[package]\nname = \"nova\"\n",
        )
        .expect("marker should be written");
        let rustc = write_script(tempdir.path(), "slow-rustc", "sleep 2\n");

        let result = collect_rust_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_millis(50),
            &rustc,
        );

        assert!(matches!(result, Err(RuntimeCollectError::TimedOut)));
    }
}
