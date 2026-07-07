//! Local user and host context segment.

use super::{SegmentContent, SegmentPart, Style, SyncSegment};
use crate::config::SegmentConfig;
use crate::state::PromptState;

pub struct UserHostSegment;

impl SyncSegment for UserHostSegment {
    fn id(&self) -> &'static str {
        "user_host"
    }

    fn render(&self, state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent> {
        let user = state.env.user.as_deref()?;
        let host = state.env.host.as_deref()?;

        Some(SegmentContent::from_parts(
            self.id(),
            vec![
                SegmentPart::new(user.to_string(), user_style(config)),
                SegmentPart::new(format!("@{host}"), Style::default()),
            ],
        ))
    }
}

fn user_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        return Style::from(&config.style);
    }

    Style {
        fg: Some("green".to_string()),
        bg: None,
        bold: false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::state::{Keymap, PromptEnv};

    #[test]
    fn renders_user_and_host_when_available() {
        let state = PromptState {
            cwd: PathBuf::from("/repo"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: PromptEnv {
                user: Some("nova".to_string()),
                host: Some("M4Pro".to_string()),
                ..PromptEnv::default()
            },
        };

        let segment = UserHostSegment
            .render(&state, &SegmentConfig::default())
            .expect("user host should render");

        assert_eq!(segment.id, "user_host");
        assert_eq!(segment.text, "nova@M4Pro");
        assert_eq!(segment.parts[0].text, "nova");
        assert_eq!(segment.parts[0].style.fg.as_deref(), Some("green"));
        assert_eq!(segment.parts[1].text, "@M4Pro");
        assert_eq!(segment.parts[1].style, Style::default());
    }

    #[test]
    fn omits_user_host_when_context_is_missing() {
        let state = PromptState {
            cwd: PathBuf::from("/repo"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: PromptEnv::default(),
        };

        assert_eq!(
            UserHostSegment.render(&state, &SegmentConfig::default()),
            None
        );
    }
}
