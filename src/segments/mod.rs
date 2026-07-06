//! Segment registry and segment implementations.

pub mod dir;
pub mod duration;
pub mod exit_status;
pub mod git;
pub mod prompt_char;
pub mod runtime;
pub mod ssh;

use crate::config::{SegmentConfig, StyleConfig};
use crate::state::PromptState;

pub trait SyncSegment {
    fn id(&self) -> &'static str;
    fn render(&self, state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentContent {
    pub id: String,
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

pub fn render_sync_segment(
    id: &str,
    state: &PromptState,
    config: &SegmentConfig,
) -> Option<SegmentContent> {
    match id {
        "dir" => dir::DirSegment.render(state, config),
        "duration" => duration::DurationSegment.render(state, config),
        "exit_status" => exit_status::ExitStatusSegment.render(state, config),
        "prompt_char" => prompt_char::PromptCharSegment.render(state, config),
        "ssh" => ssh::SshSegment.render(state, config),
        _ => None,
    }
}

fn strip_control_chars(input: &str) -> String {
    input
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}
