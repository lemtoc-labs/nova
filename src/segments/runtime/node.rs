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

const NODE_VERSION_SEGMENT_ID: &str = "node_version";
const NODE_VERSION_ICON: &str = "";
const NODE_ARGS: &[&str] = &["--version"];
const NODE_DETECT_EXTENSIONS: &[&str] = &["js", "mjs", "cjs", "ts", "mts", "cts"];
const NODE_DETECT_FILES: &[&str] = &["package.json", ".node-version", ".nvmrc"];
const NODE_EXCLUDED_FILES: &[&str] = &[
    "bunfig.toml",
    "bun.lock",
    "bun.lockb",
    "deno.json",
    "deno.jsonc",
    "deno.lock",
];
const NODE_DETECT_FOLDERS: &[&str] = &["node_modules"];
const NODE_EXCLUDED_FOLDERS: &[&str] = &["esy.lock"];

pub struct NodeSegment;

impl AsyncSegmentSpec for NodeSegment {
    fn render_ids(&self) -> &'static [&'static str] {
        &[NODE_VERSION_SEGMENT_ID]
    }

    fn primary_id(&self) -> &'static str {
        NODE_VERSION_SEGMENT_ID
    }

    fn cache_key(
        &self,
        render_id: &str,
        state: &PromptState,
        config_generation: u64,
    ) -> Option<CacheKey> {
        (render_id == self.primary_id())
            .then(|| node_cache_key(&state.cwd, state.env.path.as_deref(), config_generation))
            .flatten()
    }

    fn collect(&self, ctx: &CollectContext<'_>) -> Vec<AsyncJobSegment> {
        let Some(key) = self.cache_key(self.primary_id(), ctx.state, ctx.config_generation) else {
            return Vec::new();
        };
        let config = ctx.config.segment(self.primary_id());
        collected_version_segment(
            key,
            collect_node_version(&ctx.state.cwd, ctx.state.env.path.as_deref(), ctx.deadline),
            |version| render_node_version(version, config),
        )
    }

    fn default_ttl(&self) -> Duration {
        RUNTIME_REFRESH_TTL
    }

    fn default_timeout(&self) -> Duration {
        RUNTIME_TIMEOUT
    }
}

pub fn node_cache_key(cwd: &Path, path: Option<&str>, config_generation: u64) -> Option<CacheKey> {
    is_node_project_dir(cwd).then(|| {
        CacheKey::new(
            NODE_VERSION_SEGMENT_ID,
            runtime_cache_source(cwd.to_string_lossy(), path),
            config_generation,
        )
    })
}

pub fn is_node_project_dir(cwd: &Path) -> bool {
    current_dir_matches(
        cwd,
        RuntimeDetection {
            files: NODE_DETECT_FILES,
            excluded_files: NODE_EXCLUDED_FILES,
            folders: NODE_DETECT_FOLDERS,
            excluded_folders: NODE_EXCLUDED_FOLDERS,
            extensions: NODE_DETECT_EXTENSIONS,
        },
    )
}

pub fn collect_node_version(
    cwd: &Path,
    path: Option<&str>,
    deadline: Instant,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_node_version_with_command_and_path(cwd, deadline, Path::new("node"), path)
}

#[cfg(test)]
fn collect_node_version_with_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
) -> Result<Option<String>, RuntimeCollectError> {
    collect_node_version_with_command_and_path(cwd, deadline, command, None)
}

fn collect_node_version_with_command_and_path(
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
            args: NODE_ARGS,
            detect: is_node_project_dir,
            parse: parse_node_version,
        },
    )
}

pub fn parse_node_version(output: &str) -> Option<String> {
    let version = output
        .split_whitespace()
        .next()?
        .trim_start_matches('v')
        .trim();

    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

pub fn render_node_version(version: &str, config: &SegmentConfig) -> Option<SegmentContent> {
    if version.is_empty() {
        return None;
    }

    Some(SegmentContent::new(
        NODE_VERSION_SEGMENT_ID,
        label_with_icon(version, config, NODE_VERSION_ICON),
        node_style(config),
    ))
}

fn node_style(config: &SegmentConfig) -> Style {
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

    #[cfg(unix)]
    use super::super::write_script;
    use super::*;

    #[test]
    fn runtime_cache_key_changes_when_path_changes() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");

        let first = node_cache_key(tempdir.path(), Some("/opt/node-a/bin"), 1)
            .expect("node key should exist");
        let same = node_cache_key(tempdir.path(), Some("/opt/node-a/bin"), 1)
            .expect("node key should exist");
        let second = node_cache_key(tempdir.path(), Some("/opt/node-b/bin"), 1)
            .expect("node key should exist");
        let fallback = node_cache_key(tempdir.path(), None, 1).expect("node key should exist");

        assert_eq!(first, same);
        assert_ne!(first, second);
        assert_ne!(first, fallback);
    }

    #[test]
    fn detects_node_projects_from_starship_default_conditions() {
        for marker in ["package.json", ".node-version", ".nvmrc"] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(marker), "").expect("marker should be written");

            assert!(
                is_node_project_dir(tempdir.path()),
                "{marker} should trigger node detection"
            );
        }

        for file in [
            "index.js",
            "index.mjs",
            "index.cjs",
            "index.ts",
            "index.mts",
            "index.cts",
        ] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(file), "").expect("source file should be written");

            assert!(
                is_node_project_dir(tempdir.path()),
                "{file} should trigger node detection"
            );
        }

        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::create_dir(tempdir.path().join("node_modules"))
            .expect("node_modules should be created");

        assert!(is_node_project_dir(tempdir.path()));
    }

    #[test]
    fn excludes_node_projects_with_starship_default_negative_conditions() {
        for excluded in [
            "bunfig.toml",
            "bun.lock",
            "bun.lockb",
            "deno.json",
            "deno.jsonc",
            "deno.lock",
        ] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
            fs::write(tempdir.path().join(excluded), "").expect("excluded file should be written");

            assert!(
                !is_node_project_dir(tempdir.path()),
                "{excluded} should suppress node detection"
            );
        }

        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
        fs::create_dir(tempdir.path().join("esy.lock")).expect("esy.lock should be created");

        assert!(!is_node_project_dir(tempdir.path()));
    }

    #[test]
    fn parses_node_version_output() {
        assert_eq!(
            parse_node_version("v22.17.0\n"),
            Some("22.17.0".to_string())
        );
        assert_eq!(parse_node_version("22.17.0\n"), Some("22.17.0".to_string()));
        assert_eq!(parse_node_version("\n"), None);
    }

    #[test]
    fn renders_node_version_segment() {
        let segment = render_node_version("22.17.0", &SegmentConfig::default())
            .expect("version should render");

        assert_eq!(segment.id, "node_version");
        assert_eq!(segment.text, " 22.17.0");
        assert_eq!(segment.style.fg.as_deref(), Some("green"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_node_version_with_configured_icon() {
        let segment = render_node_version(
            "22.17.0",
            &SegmentConfig {
                icon: Some("node".to_string()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "node 22.17.0");
    }

    #[test]
    #[cfg(unix)]
    fn collects_node_version_with_timeout_bound_command() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
        let node = write_script(tempdir.path(), "node", "printf 'v22.17.0\\n'\n");

        let collected = collect_node_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_secs(5),
            &node,
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("22.17.0".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn times_out_slow_node_version_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("package.json"), "{}").expect("marker should be written");
        let node = write_script(tempdir.path(), "slow-node", "sleep 2\n");

        let result = collect_node_version_with_command(
            tempdir.path(),
            Instant::now() + Duration::from_millis(50),
            &node,
        );

        assert!(matches!(result, Err(RuntimeCollectError::TimedOut)));
    }
}
