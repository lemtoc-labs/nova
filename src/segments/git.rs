//! Git branch and status collectors.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use thiserror::Error;
use wait_timeout::ChildExt;

use crate::cache::CacheKey;
use crate::config::SegmentConfig;
use crate::segments::{
    AsyncJobSegment, AsyncSegmentFailure, AsyncSegmentSpec, CollectContext, SegmentContent, Style,
    label_with_icon,
};

const GIT_BRANCH_SEGMENT_ID: &str = "git_branch";
const GIT_BRANCH_ICON: &str = "";
const GIT_STATUS_SEGMENT_ID: &str = "git_status";
const DEFAULT_GIT_STATUS_SEPARATOR: &str = "";
const GIT_BRANCH_RENDER_IDS: &[&str] = &[GIT_BRANCH_SEGMENT_ID];
const GIT_STATUS_RENDER_IDS: &[&str] = &[GIT_STATUS_SEGMENT_ID];
const GIT_BRANCH_ARGS: &[&str] = &[
    "--no-optional-locks",
    "symbolic-ref",
    "--quiet",
    "--short",
    "HEAD",
];
const GIT_DETACHED_HEAD_ARGS: &[&str] = &["--no-optional-locks", "rev-parse", "--short", "HEAD"];
const GIT_STATUS_ARGS: &[&str] = &[
    "--no-optional-locks",
    "status",
    "--porcelain=v2",
    "--branch",
    "--show-stash",
    "-z",
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitBranch {
    pub branch: Option<String>,
    pub head_oid: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitStatus {
    pub staged: usize,
    pub modified: usize,
    pub untracked: usize,
    pub conflicted: usize,
    pub stashed: usize,
    pub ahead: usize,
    pub behind: usize,
}

impl GitStatus {
    pub fn has_changes(&self) -> bool {
        self.staged > 0
            || self.modified > 0
            || self.untracked > 0
            || self.conflicted > 0
            || self.stashed > 0
            || self.ahead > 0
            || self.behind > 0
    }
}

pub struct GitBranchSegment;

impl AsyncSegmentSpec for GitBranchSegment {
    fn render_ids(&self) -> &'static [&'static str] {
        GIT_BRANCH_RENDER_IDS
    }

    fn primary_id(&self) -> &'static str {
        GIT_BRANCH_SEGMENT_ID
    }

    fn cache_key(
        &self,
        render_id: &str,
        state: &crate::state::PromptState,
        config_generation: u64,
    ) -> Option<CacheKey> {
        match render_id {
            GIT_BRANCH_SEGMENT_ID => {
                git_cache_key(GIT_BRANCH_SEGMENT_ID, &state.cwd, config_generation)
            }
            _ => None,
        }
    }

    fn collect(&self, ctx: &CollectContext<'_>) -> Vec<AsyncJobSegment> {
        let Some(branch_key) =
            self.cache_key(GIT_BRANCH_SEGMENT_ID, ctx.state, ctx.config_generation)
        else {
            return Vec::new();
        };
        let branch_config = ctx.config.segment(GIT_BRANCH_SEGMENT_ID);

        let branch = match collect_git_branch(&ctx.state.cwd, ctx.deadline) {
            Ok(Some(branch)) => branch,
            Ok(None) => {
                return vec![AsyncJobSegment {
                    key: branch_key,
                    content: Ok(None),
                }];
            }
            Err(_error) => {
                return vec![AsyncJobSegment {
                    key: branch_key,
                    content: Err(AsyncSegmentFailure::Failed),
                }];
            }
        };

        vec![AsyncJobSegment {
            key: branch_key,
            content: Ok(render_git_branch(&branch, branch_config)),
        }]
    }

    fn default_ttl(&self) -> Duration {
        Duration::ZERO
    }

    fn default_timeout(&self) -> Duration {
        Duration::from_secs(1)
    }
}

pub struct GitStatusSegment;

impl AsyncSegmentSpec for GitStatusSegment {
    fn render_ids(&self) -> &'static [&'static str] {
        GIT_STATUS_RENDER_IDS
    }

    fn primary_id(&self) -> &'static str {
        GIT_STATUS_SEGMENT_ID
    }

    fn cache_key(
        &self,
        render_id: &str,
        state: &crate::state::PromptState,
        config_generation: u64,
    ) -> Option<CacheKey> {
        match render_id {
            GIT_STATUS_SEGMENT_ID => {
                git_cache_key(GIT_STATUS_SEGMENT_ID, &state.cwd, config_generation)
            }
            _ => None,
        }
    }

    fn collect(&self, ctx: &CollectContext<'_>) -> Vec<AsyncJobSegment> {
        let Some(status_key) =
            self.cache_key(GIT_STATUS_SEGMENT_ID, ctx.state, ctx.config_generation)
        else {
            return Vec::new();
        };
        let status_config = ctx.config.segment(GIT_STATUS_SEGMENT_ID);

        let status = match collect_git_status(&ctx.state.cwd, ctx.deadline) {
            Ok(Some(status)) => status,
            Ok(None) => {
                return vec![AsyncJobSegment {
                    key: status_key,
                    content: Ok(None),
                }];
            }
            Err(_error) => {
                return vec![AsyncJobSegment {
                    key: status_key,
                    content: Err(AsyncSegmentFailure::Failed),
                }];
            }
        };

        vec![AsyncJobSegment {
            key: status_key,
            content: Ok(render_git_status(&status, status_config)),
        }]
    }

    fn default_ttl(&self) -> Duration {
        Duration::ZERO
    }

    fn default_timeout(&self) -> Duration {
        Duration::from_secs(1)
    }
}

pub fn render_git_branch(branch: &GitBranch, config: &SegmentConfig) -> Option<SegmentContent> {
    let branch = branch
        .branch
        .clone()
        .or_else(|| branch.head_oid.as_deref().map(detached_head_label))?;
    let text = label_with_icon(&branch, config, GIT_BRANCH_ICON);

    Some(SegmentContent::new(
        GIT_BRANCH_SEGMENT_ID,
        text,
        git_branch_style(config),
    ))
}

pub fn render_git_status(status: &GitStatus, config: &SegmentConfig) -> Option<SegmentContent> {
    let text = format_git_indicators(status, config)?;
    Some(SegmentContent::new(
        GIT_STATUS_SEGMENT_ID,
        text,
        git_status_style(config),
    ))
}

#[derive(Debug, Error)]
pub enum GitCollectError {
    #[error("git command timed out")]
    TimedOut,
    #[error("failed to spawn git command: {0}")]
    Spawn(std::io::Error),
    #[error("failed to wait for git command: {0}")]
    Wait(std::io::Error),
    #[error("failed to read git command output: {0}")]
    ReadOutput(std::io::Error),
    #[error("git command output reader panicked")]
    ReaderPanicked,
    #[error("failed to capture git command output")]
    MissingStdout,
    #[error("git command exited unsuccessfully")]
    NonZeroExit,
}

fn git_cache_key(segment_id: &str, cwd: &Path, config_generation: u64) -> Option<CacheKey> {
    let root = find_repository_root(cwd)?;
    Some(CacheKey::new(
        segment_id,
        root.to_string_lossy(),
        config_generation,
    ))
}

pub fn git_branch_cache_key(cwd: &Path, config_generation: u64) -> Option<CacheKey> {
    git_cache_key(GIT_BRANCH_SEGMENT_ID, cwd, config_generation)
}

pub fn git_status_cache_key(cwd: &Path, config_generation: u64) -> Option<CacheKey> {
    git_cache_key(GIT_STATUS_SEGMENT_ID, cwd, config_generation)
}

pub fn find_repository_root(cwd: &Path) -> Option<PathBuf> {
    let mut current = cwd;

    loop {
        let marker = current.join(".git");
        if marker.is_dir() || marker.is_file() {
            return Some(current.to_path_buf());
        }

        current = current.parent()?;
    }
}

pub fn collect_git_branch(
    cwd: &Path,
    deadline: Instant,
) -> Result<Option<GitBranch>, GitCollectError> {
    collect_git_branch_with_command(cwd, deadline, Path::new("git"))
}

fn collect_git_branch_with_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
) -> Result<Option<GitBranch>, GitCollectError> {
    if find_repository_root(cwd).is_none() {
        return Ok(None);
    }

    let branch_output = match run_git_command(cwd, deadline, command, GIT_BRANCH_ARGS)? {
        GitCommandOutput::Success(output) => output,
        GitCommandOutput::NonZeroExit => {
            return collect_detached_git_branch_with_command(cwd, deadline, command);
        }
    };
    let branch = trimmed_stdout(&branch_output);

    if branch.is_empty() {
        Ok(None)
    } else {
        Ok(Some(GitBranch {
            branch: Some(branch),
            head_oid: None,
        }))
    }
}

fn collect_detached_git_branch_with_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
) -> Result<Option<GitBranch>, GitCollectError> {
    let output = match run_git_command(cwd, deadline, command, GIT_DETACHED_HEAD_ARGS)? {
        GitCommandOutput::Success(output) => output,
        GitCommandOutput::NonZeroExit => return Ok(None),
    };
    let head_oid = trimmed_stdout(&output);

    if head_oid.is_empty() {
        Ok(None)
    } else {
        Ok(Some(GitBranch {
            branch: None,
            head_oid: Some(head_oid),
        }))
    }
}

pub fn collect_git_status(
    cwd: &Path,
    deadline: Instant,
) -> Result<Option<GitStatus>, GitCollectError> {
    collect_git_status_with_command(cwd, deadline, Path::new("git"))
}

fn collect_git_status_with_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
) -> Result<Option<GitStatus>, GitCollectError> {
    if find_repository_root(cwd).is_none() {
        return Ok(None);
    }

    match run_git_command(cwd, deadline, command, GIT_STATUS_ARGS)? {
        GitCommandOutput::Success(output) => Ok(Some(parse_porcelain_v2_z(&output))),
        GitCommandOutput::NonZeroExit => Err(GitCollectError::NonZeroExit),
    }
}

enum GitCommandOutput {
    Success(Vec<u8>),
    NonZeroExit,
}

fn run_git_command(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
    args: &[&str],
) -> Result<GitCommandOutput, GitCollectError> {
    let timeout = remaining_time(deadline)?;
    let mut child = Command::new(command)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(GitCollectError::Spawn)?;
    let stdout = child.stdout.take().ok_or(GitCollectError::MissingStdout)?;
    let stdout_reader = read_stdout(stdout);

    let status = match child.wait_timeout(timeout).map_err(GitCollectError::Wait)? {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            return Err(GitCollectError::TimedOut);
        }
    };
    let output = join_stdout(stdout_reader)?;

    if !status.success() {
        return Ok(GitCommandOutput::NonZeroExit);
    }

    Ok(GitCommandOutput::Success(output))
}

fn trimmed_stdout(output: &[u8]) -> String {
    String::from_utf8_lossy(output).trim().to_string()
}

fn remaining_time(deadline: Instant) -> Result<Duration, GitCollectError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or(GitCollectError::TimedOut)?;

    if remaining.is_zero() {
        Err(GitCollectError::TimedOut)
    } else {
        Ok(remaining)
    }
}

fn read_stdout(mut stdout: std::process::ChildStdout) -> JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output)?;
        Ok(output)
    })
}

fn join_stdout(
    stdout_reader: JoinHandle<std::io::Result<Vec<u8>>>,
) -> Result<Vec<u8>, GitCollectError> {
    stdout_reader
        .join()
        .map_err(|_panic| GitCollectError::ReaderPanicked)?
        .map_err(GitCollectError::ReadOutput)
}

pub fn parse_porcelain_v2_z(output: &[u8]) -> GitStatus {
    let mut status = GitStatus::default();

    for record in output.split(|byte| *byte == b'\0') {
        if record.is_empty() {
            continue;
        }

        let record = String::from_utf8_lossy(record);
        parse_record(&record, &mut status);
    }

    status
}

fn parse_record(record: &str, status: &mut GitStatus) {
    if let Some(counts) = record.strip_prefix("# branch.ab ") {
        parse_ahead_behind(counts, status);
    } else if let Some(stash_count) = record.strip_prefix("# stash ") {
        status.stashed = stash_count.trim().parse().unwrap_or_default();
    } else if record.starts_with("1 ") || record.starts_with("2 ") {
        parse_changed_entry(record, status);
    } else if record.starts_with("u ") {
        status.conflicted += 1;
    } else if record.starts_with("? ") {
        status.untracked += 1;
    }
}

fn parse_ahead_behind(counts: &str, status: &mut GitStatus) {
    for count in counts.split_whitespace() {
        if let Some(ahead) = count.strip_prefix('+') {
            status.ahead = ahead.parse().unwrap_or_default();
        } else if let Some(behind) = count.strip_prefix('-') {
            status.behind = behind.parse().unwrap_or_default();
        }
    }
}

fn parse_changed_entry(record: &str, status: &mut GitStatus) {
    let Some(xy) = record.split_whitespace().nth(1) else {
        return;
    };

    let mut chars = xy.chars();
    let index_status = chars.next();
    let worktree_status = chars.next();

    if index_status.is_some_and(|status| status != '.') {
        status.staged += 1;
    }
    if worktree_status.is_some_and(|status| status != '.') {
        status.modified += 1;
    }
}

const DETACHED_OID_PREFIX_LEN: usize = 7;

fn detached_head_label(oid: &str) -> String {
    format!("HEAD {}", short_oid(oid))
}

fn short_oid(oid: &str) -> &str {
    &oid[..oid.len().min(DETACHED_OID_PREFIX_LEN)]
}

fn format_git_indicators(status: &GitStatus, config: &SegmentConfig) -> Option<String> {
    let mut indicators = Vec::new();
    let show_counts = config.show_counts.unwrap_or(false);
    push_indicator(
        &mut indicators,
        git_status_icon(config, "conflicted", "="),
        status.conflicted,
        show_counts,
    );
    push_indicator(
        &mut indicators,
        git_status_icon(config, "modified", "!"),
        status.modified,
        show_counts,
    );
    push_indicator(
        &mut indicators,
        git_status_icon(config, "staged", "+"),
        status.staged,
        show_counts,
    );
    push_indicator(
        &mut indicators,
        git_status_icon(config, "untracked", "?"),
        status.untracked,
        show_counts,
    );
    push_indicator(
        &mut indicators,
        git_status_icon(config, "stash", "$"),
        status.stashed,
        show_counts,
    );
    push_indicator(
        &mut indicators,
        git_status_icon(config, "ahead", "⇡"),
        status.ahead,
        show_counts,
    );
    push_indicator(
        &mut indicators,
        git_status_icon(config, "behind", "⇣"),
        status.behind,
        show_counts,
    );

    if indicators.is_empty() {
        None
    } else {
        Some(format!(
            "[{}]",
            indicators.join(
                config
                    .separator
                    .as_deref()
                    .unwrap_or(DEFAULT_GIT_STATUS_SEPARATOR),
            )
        ))
    }
}

fn git_status_icon<'a>(config: &'a SegmentConfig, name: &str, default: &'a str) -> Option<&'a str> {
    match config.icons.get(name).map(String::as_str) {
        Some("") => None,
        Some(icon) => Some(icon),
        None => Some(default),
    }
}

fn push_indicator(
    indicators: &mut Vec<String>,
    symbol: Option<&str>,
    count: usize,
    show_counts: bool,
) {
    if count > 0
        && let Some(symbol) = symbol
    {
        let indicator = if show_counts {
            format!("{symbol}{count}")
        } else {
            symbol.to_string()
        };
        indicators.push(indicator);
    }
}

fn git_branch_style(config: &SegmentConfig) -> Style {
    style_or_default(
        config,
        Style {
            fg: Some("magenta".to_string()),
            bg: None,
            bold: true,
        },
    )
}

fn git_status_style(config: &SegmentConfig) -> Style {
    style_or_default(
        config,
        Style {
            fg: Some("red".to_string()),
            bg: None,
            bold: true,
        },
    )
}

fn style_or_default(config: &SegmentConfig, default: Style) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        default
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;

    use super::*;

    #[test]
    fn renders_branch_segment() {
        let branch = GitBranch {
            branch: Some("main".to_string()),
            ..GitBranch::default()
        };

        let rendered =
            render_git_branch(&branch, &SegmentConfig::default()).expect("branch should render");

        assert_eq!(rendered.id, "git_branch");
        assert_eq!(rendered.text, " main");
        assert_eq!(rendered.style.fg.as_deref(), Some("magenta"));
        assert!(rendered.style.bold);
    }

    #[test]
    fn renders_detached_head_segment_with_short_oid() {
        let branch = GitBranch {
            head_oid: Some("abcdef0123456789".to_string()),
            ..GitBranch::default()
        };

        let rendered = render_git_branch(&branch, &SegmentConfig::default())
            .expect("detached head should render");

        assert_eq!(rendered.text, " HEAD abcdef0");
    }

    #[test]
    fn renders_branch_segment_with_configured_icon() {
        let branch = GitBranch {
            branch: Some("main".to_string()),
            ..GitBranch::default()
        };
        let config = SegmentConfig {
            icon: Some("git".to_string()),
            ..SegmentConfig::default()
        };

        let rendered = render_git_branch(&branch, &config).expect("branch should render");

        assert_eq!(rendered.text, "git main");
    }

    #[test]
    fn renders_branch_segment_without_icon_when_configured_empty() {
        let branch = GitBranch {
            branch: Some("main".to_string()),
            ..GitBranch::default()
        };
        let config = SegmentConfig {
            icon: Some(String::new()),
            ..SegmentConfig::default()
        };

        let rendered = render_git_branch(&branch, &config).expect("branch should render");

        assert_eq!(rendered.text, "main");
    }

    #[test]
    fn hides_branch_segment_when_branch_and_oid_are_absent() {
        assert_eq!(
            render_git_branch(&GitBranch::default(), &SegmentConfig::default()),
            None
        );
    }

    #[test]
    fn renders_status_indicators_in_stable_order() {
        let status = GitStatus {
            staged: 2,
            modified: 1,
            untracked: 3,
            conflicted: 4,
            stashed: 5,
            ahead: 6,
            behind: 7,
        };

        let rendered = render_git_status(&status, &SegmentConfig::default())
            .expect("dirty status should render");

        assert_eq!(rendered.id, "git_status");
        assert_eq!(rendered.text, "[=!+?$⇡⇣]");
        assert_eq!(rendered.style.fg.as_deref(), Some("red"));
        assert!(rendered.style.bold);
    }

    #[test]
    fn renders_status_indicators_with_configured_icons() {
        let status = GitStatus {
            staged: 2,
            modified: 1,
            untracked: 3,
            stashed: 4,
            ..GitStatus::default()
        };
        let config = SegmentConfig {
            icons: [
                ("staged".to_string(), "S".to_string()),
                ("untracked".to_string(), "U".to_string()),
                ("stash".to_string(), "T".to_string()),
            ]
            .into(),
            ..SegmentConfig::default()
        };

        let rendered = render_git_status(&status, &config).expect("dirty status should render");

        assert_eq!(rendered.text, "[!SUT]");
    }

    #[test]
    fn renders_status_indicators_with_configured_space_separator() {
        let status = GitStatus {
            staged: 2,
            modified: 1,
            untracked: 3,
            ..GitStatus::default()
        };
        let config = SegmentConfig {
            separator: Some(" ".to_string()),
            ..SegmentConfig::default()
        };

        let rendered = render_git_status(&status, &config).expect("dirty status should render");

        assert_eq!(rendered.text, "[! + ?]");
    }

    #[test]
    fn renders_status_indicators_with_configured_empty_separator() {
        let status = GitStatus {
            staged: 2,
            modified: 1,
            untracked: 3,
            ..GitStatus::default()
        };
        let config = SegmentConfig {
            separator: Some(String::new()),
            show_counts: Some(true),
            ..SegmentConfig::default()
        };

        let rendered = render_git_status(&status, &config).expect("dirty status should render");

        assert_eq!(rendered.text, "[!1+2?3]");
    }

    #[test]
    fn renders_status_indicators_with_configured_counts() {
        let status = GitStatus {
            staged: 2,
            modified: 1,
            untracked: 3,
            ..GitStatus::default()
        };
        let config = SegmentConfig {
            show_counts: Some(true),
            ..SegmentConfig::default()
        };

        let rendered = render_git_status(&status, &config).expect("dirty status should render");

        assert_eq!(rendered.text, "[!1+2?3]");
    }

    #[test]
    fn hides_status_indicators_with_empty_configured_icons() {
        let status = GitStatus {
            staged: 2,
            modified: 1,
            ..GitStatus::default()
        };
        let config = SegmentConfig {
            icons: [("staged".to_string(), String::new())].into(),
            ..SegmentConfig::default()
        };

        let rendered = render_git_status(&status, &config).expect("dirty status should render");

        assert_eq!(rendered.text, "[!]");
    }

    #[test]
    fn hides_status_segment_when_repository_is_clean() {
        assert_eq!(
            render_git_status(&GitStatus::default(), &SegmentConfig::default()),
            None
        );
    }

    #[test]
    fn renderers_honor_custom_styles() {
        let config = SegmentConfig {
            style: crate::config::StyleConfig {
                fg: Some("cyan".to_string()),
                bg: None,
                bold: false,
            },
            ..SegmentConfig::default()
        };
        let branch_status = GitBranch {
            branch: Some("main".to_string()),
            ..GitBranch::default()
        };
        let status = GitStatus {
            staged: 1,
            ..GitStatus::default()
        };

        let branch = render_git_branch(&branch_status, &config).expect("branch should render");
        let git_status = render_git_status(&status, &config).expect("status should render");

        assert_eq!(branch.style.fg.as_deref(), Some("cyan"));
        assert!(!branch.style.bold);
        assert_eq!(git_status.style.fg.as_deref(), Some("cyan"));
        assert!(!git_status.style.bold);
    }

    #[test]
    fn branch_and_status_use_independent_config_styles() {
        let branch_config = SegmentConfig {
            style: crate::config::StyleConfig {
                fg: Some("cyan".to_string()),
                bg: None,
                bold: false,
            },
            ..SegmentConfig::default()
        };
        let status_config = SegmentConfig {
            style: crate::config::StyleConfig {
                fg: Some("yellow".to_string()),
                bg: None,
                bold: true,
            },
            ..SegmentConfig::default()
        };
        let branch_status = GitBranch {
            branch: Some("main".to_string()),
            ..GitBranch::default()
        };
        let status = GitStatus {
            staged: 1,
            ..GitStatus::default()
        };

        let branch =
            render_git_branch(&branch_status, &branch_config).expect("branch should render");
        let git_status = render_git_status(&status, &status_config).expect("status should render");

        assert_eq!(branch.style.fg.as_deref(), Some("cyan"));
        assert!(!branch.style.bold);
        assert_eq!(git_status.style.fg.as_deref(), Some("yellow"));
        assert!(git_status.style.bold);
    }

    #[test]
    fn builds_cache_key_from_repository_root() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        init_repo(dir.path())?;
        let nested = dir.path().join("src").join("deep");
        std::fs::create_dir_all(&nested)?;

        let key = git_status_cache_key(&nested, 4).expect("repo root should be detected");

        assert_eq!(key.segment_id, "git_status");
        assert_eq!(key.source, dir.path().to_string_lossy());
        assert_eq!(key.config_generation, 4);

        let branch_key = git_branch_cache_key(&nested, 4).expect("repo root should be detected");
        assert_eq!(branch_key.segment_id, "git_branch");
        assert_eq!(branch_key.source, dir.path().to_string_lossy());
        assert_eq!(branch_key.config_generation, 4);
        Ok(())
    }

    #[test]
    fn accepts_git_file_markers_for_worktrees() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let worktree = dir.path().join("worktree");
        std::fs::create_dir(&worktree)?;
        std::fs::write(worktree.join(".git"), "gitdir: ../actual-gitdir\n")?;

        assert_eq!(find_repository_root(&worktree), Some(worktree));
        Ok(())
    }

    #[test]
    fn cache_key_is_none_outside_repository() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;

        assert_eq!(git_status_cache_key(dir.path(), 1), None);
        assert_eq!(git_branch_cache_key(dir.path(), 1), None);
        Ok(())
    }

    #[test]
    fn parses_counts_from_porcelain_v2_z() {
        let output = b"# branch.oid abc123def456\0\
# branch.head main\0\
# branch.ab +1 -2\0\
1 M. N... 000000 000000 000000 abc123 def456 staged.rs\0\
1 .M N... 000000 000000 000000 abc123 def456 modified.rs\0\
? untracked.txt\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.ahead, 1);
        assert_eq!(status.behind, 2);
        assert_eq!(status.staged, 1);
        assert_eq!(status.modified, 1);
        assert_eq!(status.untracked, 1);
        assert_eq!(status.conflicted, 0);
    }

    #[test]
    fn ignores_branch_headers_when_parsing_status() {
        let output = b"# branch.oid abc123\0# branch.head (detached)\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status, GitStatus::default());
    }

    #[test]
    fn parses_staged_and_modified_from_single_entry() {
        let output = b"# branch.head feature\0\
1 MM N... 000000 000000 000000 abc123 def456 both.rs\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.staged, 1);
        assert_eq!(status.modified, 1);
    }

    #[test]
    fn parses_rename_as_staged_change() {
        let output = b"# branch.head main\0\
2 R. N... 000000 000000 000000 abc123 def456 R100 new.rs\0old.rs\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.staged, 1);
        assert_eq!(status.modified, 0);
    }

    #[test]
    fn parses_conflicted_entries() {
        let output = b"# branch.head main\0\
u UU N... 000000 000000 000000 100644 100644 100644 abc123 def456 ghi789 conflict.rs\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.conflicted, 1);
    }

    #[test]
    fn parses_stash_count() {
        let output = b"# branch.head main\0# stash 3\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.stashed, 3);
    }

    #[test]
    fn malformed_counts_fall_back_to_zero() {
        let output = b"# branch.head main\0# branch.ab +x -y\0# stash nope\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
        assert_eq!(status.stashed, 0);
        assert!(!status.has_changes());
    }

    #[test]
    fn empty_output_returns_default_status() {
        assert_eq!(parse_porcelain_v2_z(b""), GitStatus::default());
    }

    #[test]
    fn parses_real_git_status_output_with_staged_file() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        init_repo(dir.path())?;
        std::fs::write(dir.path().join("staged.txt"), "hello")?;
        run_git(dir.path(), &["add", "staged.txt"])?;

        let output = run_git(
            dir.path(),
            &[
                "--no-optional-locks",
                "status",
                "--porcelain=v2",
                "--branch",
                "--show-stash",
                "-z",
            ],
        )?;
        let status = parse_porcelain_v2_z(&output);

        assert_eq!(status.staged, 1);
        assert!(status.has_changes());
        Ok(())
    }

    #[test]
    fn collects_real_git_status_with_staged_file() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        init_repo(dir.path())?;
        std::fs::write(dir.path().join("staged.txt"), "hello")?;
        run_git(dir.path(), &["add", "staged.txt"])?;

        let status = collect_git_status(dir.path(), Instant::now() + Duration::from_secs(5))?
            .expect("git repo should produce status");

        assert_eq!(status.staged, 1);
        assert!(status.has_changes());
        Ok(())
    }

    #[test]
    fn collects_real_git_branch() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        init_repo(dir.path())?;

        let branch = collect_git_branch(dir.path(), Instant::now() + Duration::from_secs(5))?
            .expect("git repo should produce a branch");

        assert_eq!(branch.branch.as_deref(), Some("main"));
        assert_eq!(branch.head_oid, None);
        Ok(())
    }

    #[test]
    fn collects_detached_head_as_short_oid() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        init_repo(dir.path())?;
        std::fs::write(dir.path().join("tracked.txt"), "hello")?;
        run_git(dir.path(), &["add", "tracked.txt"])?;
        run_git(
            dir.path(),
            &[
                "-c",
                "user.name=Nova",
                "-c",
                "user.email=nova@example.com",
                "commit",
                "-m",
                "initial",
            ],
        )?;
        let full_oid = String::from_utf8(run_git(dir.path(), &["rev-parse", "HEAD"])?)?;
        run_git(dir.path(), &["checkout", "--detach", "HEAD"])?;

        let branch = collect_git_branch(dir.path(), Instant::now() + Duration::from_secs(5))?
            .expect("detached git repo should produce a head oid");

        assert_eq!(branch.branch, None);
        assert_eq!(branch.head_oid.as_deref(), Some(&full_oid.trim()[..7]));
        Ok(())
    }

    #[test]
    fn status_collector_returns_none_outside_repository() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;

        let status = collect_git_status(dir.path(), Instant::now() + Duration::from_secs(5))?;

        assert_eq!(status, None);
        Ok(())
    }

    #[test]
    fn branch_collector_returns_none_outside_repository() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;

        let branch = collect_git_branch(dir.path(), Instant::now() + Duration::from_secs(5))?;

        assert_eq!(branch, None);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn collector_times_out_slow_git_command() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir()?;
        init_repo(dir.path())?;
        let fake_git = dir.path().join("git");
        std::fs::write(&fake_git, "#!/bin/sh\nsleep 2\n")?;
        let mut permissions = std::fs::metadata(&fake_git)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_git, permissions)?;

        let error = collect_git_status_with_command(
            dir.path(),
            Instant::now() + Duration::from_millis(50),
            &fake_git,
        )
        .expect_err("slow git command should time out");

        assert!(matches!(error, GitCollectError::TimedOut));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn collector_errors_on_failed_git_command() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir()?;
        init_repo(dir.path())?;
        let fake_git = dir.path().join("git");
        std::fs::write(&fake_git, "#!/bin/sh\nexit 1\n")?;
        let mut permissions = std::fs::metadata(&fake_git)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_git, permissions)?;

        collect_git_status_with_command(
            dir.path(),
            Instant::now() + Duration::from_secs(1),
            &fake_git,
        )
        .expect_err("failed git command should be an error");
        Ok(())
    }

    fn init_repo(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let init = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(path)
            .output()?;

        if init.status.success() {
            return Ok(());
        }

        run_git(path, &["init"])?;
        Ok(())
    }

    fn run_git(path: &Path, args: &[&str]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let output = Command::new("git").args(args).current_dir(path).output()?;
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(output.stdout)
    }
}
