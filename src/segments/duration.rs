//! Command duration segment.

use super::{SegmentContent, SegmentPart, Style, SyncSegment};
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

        let prefix = config.prefix.as_deref().unwrap_or("+");
        let value = format_duration_value(duration_ms);

        Some(SegmentContent::from_parts(
            self.id(),
            duration_parts(prefix, &value, config),
        ))
    }
}

pub fn format_duration(duration_ms: u64) -> String {
    format!("+{}", format_duration_value(duration_ms))
}

fn format_duration_value(duration_ms: u64) -> String {
    if duration_ms < 1_000 {
        return format!("{duration_ms}ms");
    }

    let tenths = (duration_ms + 50) / 100;
    if tenths.is_multiple_of(10) {
        format!("{}s", tenths / 10)
    } else {
        format!("{}.{:01}s", tenths / 10, tenths % 10)
    }
}

fn duration_parts(prefix: &str, value: &str, config: &SegmentConfig) -> Vec<SegmentPart> {
    let value_style = Style::from(&config.style);
    let prefix_style = if has_style(&config.prefix_style) {
        Style::from(&config.prefix_style)
    } else {
        value_style.clone()
    };

    let prefix_part =
        (!prefix.is_empty()).then(|| SegmentPart::new(prefix.to_string(), prefix_style));

    prefix_part
        .into_iter()
        .chain(std::iter::once(SegmentPart::new(
            value.to_string(),
            value_style,
        )))
        .collect()
}

fn has_style(style: &crate::config::StyleConfig) -> bool {
    style.fg.is_some() || style.bg.is_some() || style.bold
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::config::StyleConfig;
    use crate::state::Keymap;

    #[test]
    fn formats_milliseconds_and_seconds() {
        assert_eq!(format_duration(999), "+999ms");
        assert_eq!(format_duration(1_050), "+1.1s");
        assert_eq!(format_duration(2_000), "+2s");
        assert_eq!(format_duration(12_345), "+12.3s");
    }

    #[test]
    fn renders_default_plus_prefix() {
        let state = duration_state(2_000);

        let rendered = DurationSegment
            .render(&state, &SegmentConfig::default())
            .expect("duration should render");

        assert_eq!(rendered.text, "+2s");
        assert_eq!(rendered.parts[0].text, "+");
        assert_eq!(rendered.parts[1].text, "2s");
    }

    #[test]
    fn renders_configured_prefix_with_value_style() {
        let state = duration_state(2_000);
        let config = SegmentConfig {
            prefix: Some("took ".to_string()),
            style: StyleConfig {
                fg: Some("white".to_string()),
                bg: None,
                bold: false,
            },
            ..SegmentConfig::default()
        };

        let rendered = DurationSegment
            .render(&state, &config)
            .expect("duration should render");

        assert_eq!(rendered.text, "took 2s");
        assert_eq!(rendered.parts[0].text, "took ");
        assert_eq!(rendered.parts[0].style.fg.as_deref(), Some("white"));
        assert_eq!(rendered.parts[1].text, "2s");
        assert_eq!(rendered.parts[1].style.fg.as_deref(), Some("white"));
    }

    #[test]
    fn renders_configured_prefix_style_separately() {
        let state = duration_state(2_000);
        let config = SegmentConfig {
            prefix: Some("took ".to_string()),
            prefix_style: StyleConfig {
                fg: Some("cyan".to_string()),
                bg: None,
                bold: true,
            },
            style: StyleConfig {
                fg: Some("white".to_string()),
                bg: None,
                bold: false,
            },
            ..SegmentConfig::default()
        };

        let rendered = DurationSegment
            .render(&state, &config)
            .expect("duration should render");

        assert_eq!(rendered.text, "took 2s");
        assert_eq!(rendered.parts[0].style.fg.as_deref(), Some("cyan"));
        assert!(rendered.parts[0].style.bold);
        assert_eq!(rendered.parts[1].style.fg.as_deref(), Some("white"));
        assert!(!rendered.parts[1].style.bold);
    }

    #[test]
    fn renders_without_prefix_when_configured_empty() {
        let state = duration_state(2_000);
        let config = SegmentConfig {
            prefix: Some(String::new()),
            ..SegmentConfig::default()
        };

        let rendered = DurationSegment
            .render(&state, &config)
            .expect("duration should render");

        assert_eq!(rendered.text, "2s");
        assert_eq!(rendered.parts.len(), 1);
        assert_eq!(rendered.parts[0].text, "2s");
    }

    fn duration_state(duration_ms: u64) -> PromptState {
        PromptState {
            cwd: PathBuf::from("/tmp"),
            exit_status: 0,
            duration_ms: Some(duration_ms),
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        }
    }
}
