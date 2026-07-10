use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::cache::CacheKey;
use crate::config::SegmentConfig;
use crate::segments::{
    AsyncJobSegment, AsyncSegmentSpec, CollectContext, SegmentContent, Style, label_with_icon,
};
use crate::state::PromptState;

use super::detect::{RuntimeDetection, current_dir_matches};
use super::{
    RUNTIME_REFRESH_TTL, RUNTIME_TIMEOUT, RuntimeCollectError, collect_command_output,
    collected_version_segment, remaining_time, runtime_cache_source,
};

const PYTHON_VERSION_SEGMENT_ID: &str = "python_version";
const PYTHON_VERSION_ICON: &str = "";
const PYTHON_ARGS: &[&str] = &["--version"];
const PYTHON_COMMANDS: &[&str] = &["python", "python3", "python2"];
const PYTHON_DETECT_EXTENSIONS: &[&str] = &["py", "ipynb"];
const PYTHON_DETECT_FILES: &[&str] = &[
    "requirements.txt",
    ".python-version",
    "pyproject.toml",
    "Pipfile",
    "tox.ini",
    "setup.py",
    "__init__.py",
];

pub struct PythonSegment;

impl AsyncSegmentSpec for PythonSegment {
    fn render_ids(&self) -> &'static [&'static str] {
        &[PYTHON_VERSION_SEGMENT_ID]
    }

    fn primary_id(&self) -> &'static str {
        PYTHON_VERSION_SEGMENT_ID
    }

    fn cache_key(
        &self,
        render_id: &str,
        state: &PromptState,
        config_generation: u64,
    ) -> Option<CacheKey> {
        (render_id == self.primary_id())
            .then(|| {
                python_cache_key(
                    &state.cwd,
                    state.env.virtual_env.as_deref(),
                    state.env.path.as_deref(),
                    config_generation,
                )
            })
            .flatten()
    }

    fn collect(&self, ctx: &CollectContext<'_>) -> Vec<AsyncJobSegment> {
        let Some(key) = self.cache_key(self.primary_id(), ctx.state, ctx.config_generation) else {
            return Vec::new();
        };
        let config = ctx.config.segment(self.primary_id());
        collected_version_segment(
            key,
            collect_python_version(
                &ctx.state.cwd,
                ctx.state.env.virtual_env.as_deref(),
                ctx.state.env.path.as_deref(),
                ctx.deadline,
            ),
            |version| render_python_version(version, config),
        )
    }

    fn default_ttl(&self) -> Duration {
        RUNTIME_REFRESH_TTL
    }

    fn default_timeout(&self) -> Duration {
        RUNTIME_TIMEOUT
    }
}

pub fn python_cache_key(
    cwd: &Path,
    virtual_env: Option<&Path>,
    path: Option<&str>,
    config_generation: u64,
) -> Option<CacheKey> {
    is_python_project_dir(cwd, virtual_env).then(|| {
        CacheKey::new(
            PYTHON_VERSION_SEGMENT_ID,
            runtime_cache_source(python_cache_source(cwd, virtual_env), path),
            config_generation,
        )
    })
}

pub fn is_python_project_dir(cwd: &Path, virtual_env: Option<&Path>) -> bool {
    virtual_env.is_some()
        || current_dir_matches(
            cwd,
            RuntimeDetection {
                files: PYTHON_DETECT_FILES,
                excluded_files: &[],
                folders: &[],
                excluded_folders: &[],
                extensions: PYTHON_DETECT_EXTENSIONS,
            },
        )
}

pub fn collect_python_version(
    cwd: &Path,
    virtual_env: Option<&Path>,
    path: Option<&str>,
    deadline: Instant,
) -> Result<Option<String>, RuntimeCollectError> {
    let commands = python_command_paths(virtual_env);
    collect_python_version_with_commands_and_path(cwd, virtual_env, deadline, &commands, path)
}

#[cfg(test)]
fn collect_python_version_with_commands(
    cwd: &Path,
    virtual_env: Option<&Path>,
    deadline: Instant,
    commands: &[PathBuf],
) -> Result<Option<String>, RuntimeCollectError> {
    collect_python_version_with_commands_and_path(cwd, virtual_env, deadline, commands, None)
}

fn collect_python_version_with_commands_and_path(
    cwd: &Path,
    virtual_env: Option<&Path>,
    deadline: Instant,
    commands: &[PathBuf],
    path: Option<&str>,
) -> Result<Option<String>, RuntimeCollectError> {
    if !is_python_project_dir(cwd, virtual_env) {
        return Ok(None);
    }

    for command in commands {
        let timeout = remaining_time(deadline)?;
        match collect_command_output(command, PYTHON_ARGS, cwd, timeout, path) {
            Ok(output) => {
                if let Some(version) = parse_python_version(&String::from_utf8_lossy(&output)) {
                    return Ok(Some(version));
                }
            }
            Err(RuntimeCollectError::Spawn(error))
                if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }

    Ok(None)
}

pub fn parse_python_version(output: &str) -> Option<String> {
    output.split_whitespace().nth(1).map(ToString::to_string)
}

pub fn render_python_version(version: &str, config: &SegmentConfig) -> Option<SegmentContent> {
    if version.is_empty() {
        return None;
    }

    Some(SegmentContent::new(
        PYTHON_VERSION_SEGMENT_ID,
        label_with_icon(version, config, PYTHON_VERSION_ICON),
        python_style(config),
    ))
}

fn python_cache_source(cwd: &Path, virtual_env: Option<&Path>) -> String {
    match virtual_env {
        Some(virtual_env) => format!(
            "{}|venv={}",
            cwd.to_string_lossy(),
            virtual_env.to_string_lossy()
        ),
        None => cwd.to_string_lossy().into_owned(),
    }
}

fn python_command_paths(virtual_env: Option<&Path>) -> Vec<PathBuf> {
    virtual_env
        .map(|virtual_env| virtual_env.join("bin").join("python"))
        .into_iter()
        .chain(PYTHON_COMMANDS.iter().map(PathBuf::from))
        .collect()
}

fn python_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("yellow".to_string()),
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
    fn detects_python_projects_from_starship_default_conditions() {
        for marker in [
            "requirements.txt",
            ".python-version",
            "pyproject.toml",
            "Pipfile",
            "tox.ini",
            "setup.py",
            "__init__.py",
        ] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(marker), "").expect("marker should be written");

            assert!(
                is_python_project_dir(tempdir.path(), None),
                "{marker} should trigger python detection"
            );
        }

        for file in ["main.py", "notebook.ipynb"] {
            let tempdir = tempfile::tempdir().expect("tempdir should be created");
            fs::write(tempdir.path().join(file), "").expect("source file should be written");

            assert!(
                is_python_project_dir(tempdir.path(), None),
                "{file} should trigger python detection"
            );
        }
    }

    #[test]
    fn detects_python_projects_from_virtual_env() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let virtual_env = tempdir.path().join(".venv");

        assert!(is_python_project_dir(tempdir.path(), Some(&virtual_env)));
    }

    #[test]
    fn parses_python_version_output() {
        assert_eq!(
            parse_python_version("Python 3.12.4\n"),
            Some("3.12.4".to_string())
        );
        assert_eq!(
            parse_python_version("Python 3.12.4 :: Anaconda, Inc.\n"),
            Some("3.12.4".to_string())
        );
        assert_eq!(parse_python_version("not-enough\n"), None);
    }

    #[test]
    fn renders_python_version_segment() {
        let segment = render_python_version("3.12.4", &SegmentConfig::default())
            .expect("version should render");

        assert_eq!(segment.id, "python_version");
        assert_eq!(segment.text, " 3.12.4");
        assert_eq!(segment.style.fg.as_deref(), Some("yellow"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_python_version_with_configured_icon() {
        let segment = render_python_version(
            "3.12.4",
            &SegmentConfig {
                icon: Some("py".to_string()),
                ..SegmentConfig::default()
            },
        )
        .expect("version should render");

        assert_eq!(segment.text, "py 3.12.4");
    }

    #[test]
    #[cfg(unix)]
    fn collects_python_version_with_timeout_bound_command() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("requirements.txt"), "").expect("marker should be written");
        let python = write_script(tempdir.path(), "python", "printf 'Python 3.12.4\\n'\n");

        let collected = collect_python_version_with_commands(
            tempdir.path(),
            None,
            Instant::now() + Duration::from_secs(5),
            &[python],
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("3.12.4".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn collects_python_version_from_virtual_env_command_first() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let virtual_env = tempdir.path().join(".venv");
        let bin_dir = virtual_env.join("bin");
        fs::create_dir_all(&bin_dir).expect("venv bin should be created");
        let venv_python = write_script(&bin_dir, "python", "printf 'Python 3.12.4\\n'\n");
        let fallback_python = write_script(tempdir.path(), "python", "printf 'Python 3.11.9\\n'\n");

        let collected = collect_python_version_with_commands(
            tempdir.path(),
            Some(&virtual_env),
            Instant::now() + Duration::from_secs(5),
            &[venv_python, fallback_python],
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("3.12.4".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn falls_back_to_later_python_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("requirements.txt"), "").expect("marker should be written");
        let python3 = write_script(tempdir.path(), "python3", "printf 'Python 3.12.4\\n'\n");

        let collected = collect_python_version_with_commands(
            tempdir.path(),
            None,
            Instant::now() + Duration::from_secs(5),
            &[tempdir.path().join("missing-python"), python3],
        )
        .expect("collector should succeed");

        assert_eq!(collected, Some("3.12.4".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn times_out_slow_python_version_commands() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        fs::write(tempdir.path().join("requirements.txt"), "").expect("marker should be written");
        let python = write_script(tempdir.path(), "slow-python", "sleep 2\n");

        let result = collect_python_version_with_commands(
            tempdir.path(),
            None,
            Instant::now() + Duration::from_millis(50),
            &[python],
        );

        assert!(matches!(result, Err(RuntimeCollectError::TimedOut)));
    }
}
