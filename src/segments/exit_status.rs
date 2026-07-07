//! Last command exit status segment.

use super::{SegmentContent, Style, SyncSegment};
use crate::config::SegmentConfig;
use crate::state::PromptState;

pub struct ExitStatusSegment;

impl SyncSegment for ExitStatusSegment {
    fn id(&self) -> &'static str {
        "exit_status"
    }

    fn render(&self, state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent> {
        if state.exit_status == 0 {
            return None;
        }

        Some(SegmentContent::new(
            self.id(),
            format!("[{}]", state.exit_status),
            exit_status_style(config),
        ))
    }
}

fn exit_status_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        return Style::from(&config.style);
    }

    Style {
        fg: Some("red".to_string()),
        bg: None,
        bold: true,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::state::Keymap;

    #[test]
    fn hides_success_status() {
        let segment = ExitStatusSegment;
        let state = PromptState {
            cwd: PathBuf::from("/tmp"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };

        assert_eq!(segment.render(&state, &SegmentConfig::default()), None);
    }

    #[test]
    fn renders_failed_status_in_brackets() {
        let segment = ExitStatusSegment;
        let state = PromptState {
            cwd: PathBuf::from("/tmp"),
            exit_status: 127,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };

        let rendered = segment
            .render(&state, &SegmentConfig::default())
            .expect("failed status should render");

        assert_eq!(rendered.id, "exit_status");
        assert_eq!(rendered.text, "[127]");
        assert_eq!(rendered.style.fg.as_deref(), Some("red"));
        assert!(rendered.style.bold);
    }
}
