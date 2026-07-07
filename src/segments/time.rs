//! Prompt render time segment.

use super::{SegmentContent, Style, SyncSegment};
use crate::config::SegmentConfig;
use crate::state::PromptState;

pub struct TimeSegment;

impl SyncSegment for TimeSegment {
    fn id(&self) -> &'static str {
        "time"
    }

    fn render(&self, state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent> {
        let time = state.time.as_deref()?;
        Some(SegmentContent::new(
            self.id(),
            time.to_string(),
            Style::from(&config.style),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::state::Keymap;

    #[test]
    fn renders_prompt_time_when_available() {
        let state = PromptState {
            cwd: PathBuf::from("/repo"),
            exit_status: 0,
            duration_ms: None,
            time: Some("11:16:42".to_string()),
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };

        let segment = TimeSegment
            .render(&state, &SegmentConfig::default())
            .expect("time should render");

        assert_eq!(segment.id, "time");
        assert_eq!(segment.text, "11:16:42");
    }

    #[test]
    fn omits_prompt_time_when_unavailable() {
        let state = PromptState {
            cwd: PathBuf::from("/repo"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };

        assert_eq!(TimeSegment.render(&state, &SegmentConfig::default()), None);
    }
}
