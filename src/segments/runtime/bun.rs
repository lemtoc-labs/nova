use std::path::Path;
use std::time::{Duration, Instant};

use crate::cache::CacheKey;
use crate::config::SegmentConfig;
use crate::segments::{
    AsyncJobSegment, AsyncSegmentSpec, CollectContext, SegmentContent, Style, label_with_icon,
};
use crate::state::PromptState;

use super::detect::{RuntimeDetection, current_dir_matches};
use super::{
    RUNTIME_REFRESH_TTL, RUNTIME_TIMEOUT, RuntimeCollectError, RuntimeCommandSpec,
    collect_detected_command_version, collected_version_segment, runtime_cache_source,
};

const BUN_VERSION_SEGMENT_ID: &str = "bun_version";
const BUN_VERSION_ICON: &str = "";
const BUN_ARGS: &[&str] = &["--version"];
const BUN_DETECT_FILES: &[&str] = &["bun.lock", "bun.lockb", "bunfig.toml"];

pub struct BunSegment;

impl AsyncSegmentSpec for BunSegment {
    fn render_ids(&self) -> &'static [&'static str] {
        &[BUN_VERSION_SEGMENT_ID]
    }

    fn primary_id(&self) -> &'static str {
        BUN_VERSION_SEGMENT_ID
    }

    fn cache_key(
        &self,
        render_id: &str,
        state: &PromptState,
        config_generation: u64,
    ) -> Option<CacheKey> {
        (render_id == self.primary_id())
            .then(|| bun_cache_key(&state.cwd, state.env.path.as_deref(), config_generation))
            .flatten()
    }

    fn collect(&self, ctx: &CollectContext<'_>) -> Vec<AsyncJobSegment> {
        let Some(key) = self.cache_key(self.primary_id(), ctx.state, ctx.config_generation) else {
            return Vec::new();
        };
        let config = ctx.config.segment(self.primary_id());
        collected_version_segment(
            key,
            collect_bun_version(&ctx.state.cwd, ctx.state.env.path.as_deref(), ctx.deadline),
            |version| render_bun_version(version, config),
        )
    }

    fn default_ttl(&self) -> Duration {
        RUNTIME_REFRESH_TTL
    }

    fn default_timeout(&self) -> Duration {
        RUNTIME_TIMEOUT
    }
}

pub fn bun_cache_key(cwd: &Path, path: Option<&str>, config_generation: u64) -> Option<CacheKey> {
    is_bun_project_dir(cwd).then(|| {
        CacheKey::new(
            BUN_VERSION_SEGMENT_ID,
            runtime_cache_source(cwd.to_string_lossy(), path),
            config_generation,
        )
    })
}

pub fn is_bun_project_dir(cwd: &Path) -> bool {
    current_dir_matches(
        cwd,
        RuntimeDetection {
            files: BUN_DETECT_FILES,
            excluded_files: &[],
            folders: &[],
            excluded_folders: &[],
            extensions: &[],
        },
    )
}

pub fn collect_bun_version(
    cwd: &Path,
    path: Option<&str>,
    deadline: Instant,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_bun_version_with_command_and_path(cwd, deadline, Path::new("bun"), path)
}

#[cfg(test)]
fn collect_bun_version_with_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_bun_version_with_command_and_path(cwd, deadline, command, None)
}

fn collect_bun_version_with_command_and_path(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
    path: Option<&str>,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_detected_command_version(
        cwd,
        deadline,
        command,
        path,
        RuntimeCommandSpec {
            args: BUN_ARGS,
            detect: is_bun_project_dir,
            parse: parse_bun_version,
        },
    )
}

pub fn parse_bun_version(output: &str) -> Option<String> {
    let version = output.trim();

    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

pub fn render_bun_version(version: &str, config: &SegmentConfig) -> Option<SegmentContent> {
    if version.is_empty() {
        return None;
    }

    Some(SegmentContent::new(
        BUN_VERSION_SEGMENT_ID,
        label_with_icon(version, config, BUN_VERSION_ICON),
        bun_style(config),
    ))
}

fn bun_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("red".to_string()),
            bg: None,
            bold: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::node::is_node_project_dir;
    #[cfg(unix)]
    use super::super::write_script;
    use super::*;

    #[test]
    fn detects_bun_projects_from_starship_default_conditions() {
        for marker in ["bun.lock", "bun.lockb", "bunfig.toml"] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(marker), "").expect("marker should be written");

            assert!(
                is_bun_project_dir(tempdir.path()),
                "{marker} should trigger bun detection"
            );
        }
    }

    #[test]
    fn bun_projects_suppress_node_detection() {
        for marker in ["bun.lock", "bun.lockb", "bunfig.toml"] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
            fs::write(tempdir.path().join(marker), "").expect("bun marker should be written");

            assert!(
                !is_node_project_dir(tempdir.path()),
                "{marker} should suppress node detection"
            );
            assert!(
                is_bun_project_dir(tempdir.path()),
                "{marker} should trigger bun detection"
            );
        }
    }

    #[test]
    fn parses_bun_version_output() {
        assert_eq!(parse_bun_version("1.2.18\n"), Some("1.2.18".to_string()));
        assert_eq!(parse_bun_version("\n"), None);
    }

    #[test]
    fn renders_bun_version_segment() {
        let segment =
            render_bun_version("1.2.18", &SegmentConfig::default()).expect("version should render");

        assert_eq!(segment.id, "bun_version");
        assert_eq!(segment.text, " 1.2.18");
        assert_eq!(segment.style.fg.as_deref(), Some("red"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_bun_version_with_configured_icon() {
        let segment = render_bun_version(
            "1.2.18",
            &SegmentConfig {
                icon: Some("bun".to_string()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "bun 1.2.18");
    }

    #[test]
    #[cfg(unix)]
    fn collects_bun_version_with_timeout_bound_command() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("bun.lock"), "").expect("marker should be written");
        let bun = write_script(tempdir.path(), "bun", "printf '1.2.18\\n'\n");

        let collected = collect_bun_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_secs(5),
            &bun,
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("1.2.18".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn times_out_slow_bun_version_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("bun.lock"), "").expect("marker should be written");
        let bun = write_script(tempdir.path(), "slow-bun", "sleep 2\n");

        let result = collect_bun_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_millis(50),
            &bun,
        );

        assert!(matches!(result, Err(RuntimeCollectError::TimedOut)));
    }
}
