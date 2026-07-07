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
        let symbol = prompt_symbol(state.keymap, config);
        let text = format!("{symbol} ");

        Some(SegmentContent::new(self.id(), text, style))
    }
}

fn prompt_symbol(keymap: Keymap, config: &SegmentConfig) -> &str {
    match keymap {
        Keymap::Main => config
            .characters
            .get("main")
            .or(config.character.as_ref())
            .map(String::as_str)
            .unwrap_or("❯"),
        Keymap::ViCommand => config
            .characters
            .get("vi_command")
            .or_else(|| config.characters.get("vicmd"))
            .or(config.character.as_ref())
            .map(String::as_str)
            .unwrap_or("❮"),
    }
}

fn prompt_style(state: &PromptState, config: &SegmentConfig) -> Style {
    if state.exit_status != 0 && has_style(&config.error_style) {
        return Style::from(&config.error_style);
    }

    if has_style(&config.style) {
        return Style::from(&config.style);
    }

    if state.exit_status != 0 {
        return Style {
            fg: Some("red".to_string()),
            bg: None,
            bold: true,
        };
    }

    Style::default()
}

fn has_style(style: &crate::config::StyleConfig) -> bool {
    style.fg.is_some() || style.bg.is_some() || style.bold
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn uses_default_style_for_success_without_config() {
        let segment = PromptCharSegment;
        let state = PromptState {
            cwd: PathBuf::from("/tmp"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };

        let rendered = segment
            .render(&state, &SegmentConfig::default())
            .expect("prompt char should render");

        assert_eq!(rendered.text, "❯ ");
        assert_eq!(rendered.style, Style::default());
    }

    #[test]
    fn uses_red_style_for_failure_without_config() {
        let segment = PromptCharSegment;
        let state = PromptState {
            cwd: PathBuf::from("/tmp"),
            exit_status: 1,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };

        let rendered = segment
            .render(&state, &SegmentConfig::default())
            .expect("prompt char should render");

        assert_eq!(rendered.text, "❯ ");
        assert_eq!(rendered.style.fg.as_deref(), Some("red"));
        assert!(rendered.style.bold);
    }

    #[test]
    fn configured_prompt_style_overrides_default_failure_style() {
        let segment = PromptCharSegment;
        let state = PromptState {
            cwd: PathBuf::from("/tmp"),
            exit_status: 1,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };
        let config = SegmentConfig {
            style: crate::config::StyleConfig {
                fg: Some("green".to_string()),
                bg: None,
                bold: true,
            },
            ..SegmentConfig::default()
        };

        let rendered = segment
            .render(&state, &config)
            .expect("prompt char should render");

        assert_eq!(rendered.text, "❯ ");
        assert_eq!(rendered.style.fg.as_deref(), Some("green"));
        assert!(rendered.style.bold);
    }

    #[test]
    fn configured_error_style_overrides_prompt_style_on_failure() {
        let segment = PromptCharSegment;
        let state = PromptState {
            cwd: PathBuf::from("/tmp"),
            exit_status: 1,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };
        let config = SegmentConfig {
            style: crate::config::StyleConfig {
                fg: Some("green".to_string()),
                bg: None,
                bold: false,
            },
            error_style: crate::config::StyleConfig {
                fg: Some("red".to_string()),
                bg: None,
                bold: true,
            },
            ..SegmentConfig::default()
        };

        let rendered = segment
            .render(&state, &config)
            .expect("prompt char should render");

        assert_eq!(rendered.text, "❯ ");
        assert_eq!(rendered.style.fg.as_deref(), Some("red"));
        assert!(rendered.style.bold);
    }

    #[test]
    fn appends_gap_to_configured_prompt_character() {
        let segment = PromptCharSegment;
        let state = PromptState {
            cwd: PathBuf::from("/tmp"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };
        let config = SegmentConfig {
            character: Some(">_".to_string()),
            ..SegmentConfig::default()
        };

        let rendered = segment
            .render(&state, &config)
            .expect("prompt char should render");

        assert_eq!(rendered.text, ">_ ");
    }

    #[test]
    fn appends_gap_to_configured_vi_prompt_character() {
        let segment = PromptCharSegment;
        let state = PromptState {
            cwd: PathBuf::from("/tmp"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::ViCommand,
            env: Default::default(),
        };
        let config = SegmentConfig {
            characters: [("vi_command".to_string(), "%".to_string())].into(),
            ..SegmentConfig::default()
        };

        let rendered = segment
            .render(&state, &config)
            .expect("prompt char should render");

        assert_eq!(rendered.text, "% ");
    }
}
