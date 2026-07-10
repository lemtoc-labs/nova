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

const DENO_VERSION_SEGMENT_ID: &str = "deno_version";
const DENO_VERSION_ICON: &str = "";
const DENO_ARGS: &[&str] = &["-V"];
const DENO_DETECT_FILES: &[&str] = &[
    "deno.json",
    "deno.jsonc",
    "deno.lock",
    "mod.ts",
    "deps.ts",
    "mod.js",
    "deps.js",
];

pub struct DenoSegment;

impl AsyncSegmentSpec for DenoSegment {
    fn render_ids(&self) -> &'static [&'static str] {
        &[DENO_VERSION_SEGMENT_ID]
    }

    fn primary_id(&self) -> &'static str {
        DENO_VERSION_SEGMENT_ID
    }

    fn cache_key(
        &self,
        render_id: &str,
        state: &PromptState,
        config_generation: u64,
    ) -> Option<CacheKey> {
        (render_id == self.primary_id())
            .then(|| deno_cache_key(&state.cwd, state.env.path.as_deref(), config_generation))
            .flatten()
    }

    fn collect(&self, ctx: &CollectContext<'_>) -> Vec<AsyncJobSegment> {
        let Some(key) = self.cache_key(self.primary_id(), ctx.state, ctx.config_generation) else {
            return Vec::new();
        };
        let config = ctx.config.segment(self.primary_id());
        collected_version_segment(
            key,
            collect_deno_version(&ctx.state.cwd, ctx.state.env.path.as_deref(), ctx.deadline),
            |version| render_deno_version(version, config),
        )
    }

    fn default_ttl(&self) -> Duration {
        RUNTIME_REFRESH_TTL
    }

    fn default_timeout(&self) -> Duration {
        RUNTIME_TIMEOUT
    }
}

pub fn deno_cache_key(cwd: &Path, path: Option<&str>, config_generation: u64) -> Option<CacheKey> {
    is_deno_project_dir(cwd).then(|| {
        CacheKey::new(
            DENO_VERSION_SEGMENT_ID,
            runtime_cache_source(cwd.to_string_lossy(), path),
            config_generation,
        )
    })
}

pub fn is_deno_project_dir(cwd: &Path) -> bool {
    current_dir_matches(
        cwd,
        RuntimeDetection {
            files: DENO_DETECT_FILES,
            excluded_files: &[],
            folders: &[],
            excluded_folders: &[],
            extensions: &[],
        },
    )
}

pub fn collect_deno_version(
    cwd: &Path,
    path: Option<&str>,
    deadline: Instant,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_deno_version_with_command_and_path(cwd, deadline, Path::new("deno"), path)
}

#[cfg(test)]
fn collect_deno_version_with_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_deno_version_with_command_and_path(cwd, deadline, command, None)
}

fn collect_deno_version_with_command_and_path(
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
            args: DENO_ARGS,
            detect: is_deno_project_dir,
            parse: parse_deno_version,
        },
    )
}

pub fn parse_deno_version(output: &str) -> Option<String> {
    output.split_whitespace().nth(1).map(ToString::to_string)
}

pub fn render_deno_version(version: &str, config: &SegmentConfig) -> Option<SegmentContent> {
    if version.is_empty() {
        return None;
    }

    Some(SegmentContent::new(
        DENO_VERSION_SEGMENT_ID,
        label_with_icon(version, config, DENO_VERSION_ICON),
        deno_style(config),
    ))
}

fn deno_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("green".to_string()),
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
    fn detects_deno_projects_from_starship_default_conditions() {
        for marker in [
            "deno.json",
            "deno.jsonc",
            "deno.lock",
            "mod.ts",
            "deps.ts",
            "mod.js",
            "deps.js",
        ] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(marker), "").expect("marker should be written");

            assert!(
                is_deno_project_dir(tempdir.path()),
                "{marker} should trigger deno detection"
            );
        }
    }

    #[test]
    fn deno_projects_suppress_node_detection() {
        for marker in ["deno.json", "deno.jsonc", "deno.lock"] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
            fs::write(tempdir.path().join(marker), "").expect("deno marker should be written");

            assert!(
                !is_node_project_dir(tempdir.path()),
                "{marker} should suppress node detection"
            );
            assert!(
                is_deno_project_dir(tempdir.path()),
                "{marker} should trigger deno detection"
            );
        }
    }

    #[test]
    fn parses_deno_version_output() {
        assert_eq!(
            parse_deno_version("deno 2.3.6\n"),
            Some("2.3.6".to_string())
        );
        assert_eq!(parse_deno_version("not-enough\n"), None);
    }

    #[test]
    fn renders_deno_version_segment() {
        let segment =
            render_deno_version("2.3.6", &SegmentConfig::default()).expect("version should render");

        assert_eq!(segment.id, "deno_version");
        assert_eq!(segment.text, " 2.3.6");
        assert_eq!(segment.style.fg.as_deref(), Some("green"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_deno_version_with_configured_icon() {
        let segment = render_deno_version(
            "2.3.6",
            &SegmentConfig {
                icon: Some("deno".to_string()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "deno 2.3.6");
    }

    #[test]
    #[cfg(unix)]
    fn collects_deno_version_with_timeout_bound_command() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("deno.json"), "{}").expect("marker should be written");
        let deno = write_script(tempdir.path(), "deno", "printf 'deno 2.3.6\\n'\n");

        let collected = collect_deno_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_secs(5),
            &deno,
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("2.3.6".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn times_out_slow_deno_version_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("deno.json"), "{}").expect("marker should be written");
        let deno = write_script(tempdir.path(), "slow-deno", "sleep 2\n");

        let result = collect_deno_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_millis(50),
            &deno,
        );

        assert!(matches!(result, Err(RuntimeCollectError::TimedOut)));
    }
}
