//! SSH context segment.

use std::env;

use super::{SegmentContent, SegmentPart, Style, SyncSegment};
use crate::config::SegmentConfig;
use crate::state::PromptState;

pub struct SshSegment;

impl SyncSegment for SshSegment {
    fn id(&self) -> &'static str {
        "ssh"
    }

    fn render(&self, state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent> {
        if env::var_os("SSH_CONNECTION").is_none() && env::var_os("SSH_CLIENT").is_none() {
            return None;
        }

        let user = state
            .env
            .user
            .clone()
            .or_else(|| env::var("USER").ok())
            .unwrap_or_else(|| "unknown".to_string());
        let host = state
            .env
            .host
            .clone()
            .or_else(|| env::var("HOST").ok())
            .or_else(|| env::var("HOSTNAME").ok())
            .unwrap_or_else(|| "host".to_string());

        Some(SegmentContent::from_parts(
            self.id(),
            vec![
                SegmentPart::new(user, user_style(config)),
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
