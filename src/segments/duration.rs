//! Command duration segment.

use super::{SegmentContent, Style, SyncSegment};
use crate::config::SegmentConfig;
use crate::state::PromptState;

pub struct DurationSegment;

impl SyncSegment for DurationSegment {
    fn id(&self) -> &'static str {
        "duration"
    }

    fn render(&self, state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent> {
        let duration_ms = state.duration_ms?;
        let min_ms = config.min_ms.unwrap_or(2_000);

        if duration_ms < min_ms {
            return None;
        }

        Some(SegmentContent::new(
            self.id(),
            format_duration(duration_ms),
            Style::from(&config.style),
        ))
    }
}

pub fn format_duration(duration_ms: u64) -> String {
    if duration_ms < 1_000 {
        return format!("{duration_ms}ms");
    }

    let tenths = (duration_ms + 50) / 100;
    format!("{}.{:01}s", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_milliseconds_and_seconds() {
        assert_eq!(format_duration(999), "999ms");
        assert_eq!(format_duration(1_050), "1.1s");
        assert_eq!(format_duration(12_345), "12.3s");
    }
}
