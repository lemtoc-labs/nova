//! SSH context segment.

use std::env;

use super::{SegmentContent, Style, SyncSegment};
use crate::config::SegmentConfig;
use crate::state::PromptState;

pub struct SshSegment;

impl SyncSegment for SshSegment {
    fn id(&self) -> &'static str {
        "ssh"
    }

    fn render(&self, _state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent> {
        if env::var_os("SSH_CONNECTION").is_none() && env::var_os("SSH_CLIENT").is_none() {
            return None;
        }

        Some(SegmentContent::new(
            self.id(),
            ssh_label(),
            Style::from(&config.style),
        ))
    }
}

fn ssh_label() -> String {
    let user = env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    let host = env::var("HOSTNAME").unwrap_or_else(|_| "host".to_string());
    format!("{user}@{host}")
}
