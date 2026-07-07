//! Segment registry and segment implementations.

pub mod dir;
pub mod duration;
pub mod exit_status;
pub mod git;
pub mod prompt_char;
pub mod runtime;
pub mod ssh;
pub mod time;
pub mod user_host;

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use crate::cache::CacheKey;
use crate::config::{Config, SegmentConfig, StyleConfig};
use crate::state::PromptState;

pub trait SyncSegment: Sync {
    fn id(&self) -> &'static str;
    fn render(&self, state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent>;
}

pub struct CollectContext<'a> {
    pub state: &'a PromptState,
    pub config: &'a Config,
    pub config_generation: u64,
    pub deadline: Instant,
}

pub trait AsyncSegmentSpec: Send + Sync {
    fn render_ids(&self) -> &'static [&'static str];
    fn primary_id(&self) -> &'static str;
    fn cache_key(
        &self,
        render_id: &str,
        state: &PromptState,
        config_generation: u64,
    ) -> Option<CacheKey>;
    fn collect(&self, ctx: &CollectContext<'_>) -> Vec<AsyncJobSegment>;
    fn default_ttl(&self) -> Duration;
    fn default_timeout(&self) -> Duration;
}

#[derive(Debug)]
pub struct AsyncJobSegment {
    pub key: CacheKey,
    pub content: Result<Option<SegmentContent>, AsyncSegmentFailure>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsyncSegmentFailure {
    Failed,
}

pub static SYNC_SEGMENTS: &[&dyn SyncSegment] = &[
    &ssh::SshSegment,
    &dir::DirSegment,
    &runtime::NixShellSegment,
    &runtime::AwsSegment,
    &duration::DurationSegment,
    &time::TimeSegment,
    &exit_status::ExitStatusSegment,
    &prompt_char::PromptCharSegment,
    &user_host::UserHostSegment,
];

pub static ASYNC_SEGMENTS: &[&dyn AsyncSegmentSpec] = &[
    &git::GitSegment,
    &runtime::RustSegment,
    &runtime::BunSegment,
    &runtime::DenoSegment,
    &runtime::NodeSegment,
    &runtime::PythonSegment,
];

static KNOWN_SEGMENT_IDS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut ids = SYNC_SEGMENTS
        .iter()
        .map(|segment| segment.id())
        .collect::<Vec<_>>();
    for segment in ASYNC_SEGMENTS {
        ids.extend_from_slice(segment.render_ids());
    }
    ids.sort_unstable();
    ids.dedup();
    ids
});

pub fn known_segment_ids() -> &'static [&'static str] {
    KNOWN_SEGMENT_IDS.as_slice()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentContent {
    pub id: String,
    pub text: String,
    pub style: Style,
    pub parts: Vec<SegmentPart>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentPart {
    pub text: String,
    pub style: Style,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub bold: bool,
}

impl SegmentContent {
    pub fn new(id: impl Into<String>, text: impl Into<String>, style: Style) -> Self {
        Self {
            id: id.into(),
            text: strip_control_chars(&text.into()),
            style,
            parts: Vec::new(),
        }
    }

    pub fn from_parts(id: impl Into<String>, parts: Vec<SegmentPart>) -> Self {
        Self {
            id: id.into(),
            text: parts.iter().map(|part| part.text.as_str()).collect(),
            style: Style::default(),
            parts,
        }
    }

    pub fn uses_parts(&self) -> bool {
        !self.parts.is_empty()
            && self.text
                == self
                    .parts
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<String>()
    }
}

impl SegmentPart {
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: strip_control_chars(&text.into()),
            style,
        }
    }
}

impl From<&StyleConfig> for Style {
    fn from(config: &StyleConfig) -> Self {
        Self {
            fg: config.fg.clone(),
            bg: config.bg.clone(),
            bold: config.bold,
        }
    }
}

pub fn label_with_icon(text: &str, config: &SegmentConfig, default_icon: &str) -> String {
    match config.icon.as_deref() {
        Some("") => text.to_string(),
        Some(icon) => format!("{icon} {text}"),
        None => format!("{default_icon} {text}"),
    }
}

pub fn render_sync_segment(
    id: &str,
    state: &PromptState,
    config: &SegmentConfig,
) -> Option<SegmentContent> {
    SYNC_SEGMENTS
        .iter()
        .find(|segment| segment.id() == id)
        .and_then(|segment| segment.render(state, config))
}

fn strip_control_chars(input: &str) -> String {
    input
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}
