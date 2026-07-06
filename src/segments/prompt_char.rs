//! Prompt character segment.

use super::{SegmentContent, Style, SyncSegment};
use crate::config::SegmentConfig;
use crate::state::{Keymap, PromptState};

pub struct PromptCharSegment;

impl SyncSegment for PromptCharSegment {
    fn id(&self) -> &'static str {
        "prompt_char"
    }

    fn render(&self, state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent> {
        let style = prompt_style(state, config);
        let symbol = match state.keymap {
            Keymap::Main => "❯",
            Keymap::ViCommand => "❮",
        };

        Some(SegmentContent::new(self.id(), symbol, style))
    }
}

fn prompt_style(state: &PromptState, config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        return Style::from(&config.style);
    }

    let fg = match (state.keymap, state.exit_status) {
        (Keymap::ViCommand, _) => "yellow",
        (_, 0) => "green",
        _ => "red",
    };

    Style {
        fg: Some(fg.to_string()),
        bg: None,
        bold: true,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn changes_style_for_failed_commands() {
        let segment = PromptCharSegment;
        let state = PromptState {
            cwd: PathBuf::from("/tmp"),
            exit_status: 1,
            duration_ms: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };

        let rendered = segment
            .render(&state, &SegmentConfig::default())
            .expect("prompt char should render");

        assert_eq!(rendered.text, "❯");
        assert_eq!(rendered.style.fg.as_deref(), Some("red"));
    }
}
