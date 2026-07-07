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
    match id {
        "dir" => dir::DirSegment.render(state, config),
        "duration" => duration::DurationSegment.render(state, config),
        "exit_status" => exit_status::ExitStatusSegment.render(state, config),
        "aws" => runtime::render_aws(&state.env, config),
        "nix_shell" => runtime::render_nix_shell(&state.env, config),
        "prompt_char" => prompt_char::PromptCharSegment.render(state, config),
        "ssh" => ssh::SshSegment.render(state, config),
        "time" => time::TimeSegment.render(state, config),
        "user_host" => user_host::UserHostSegment.render(state, config),
        _ => None,
    }
}

fn strip_control_chars(input: &str) -> String {
    input
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}
