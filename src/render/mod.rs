//! Prompt composition and lowering.

pub mod width;
pub mod zsh;

use std::collections::BTreeMap;

use crate::cache::AsyncValue;
use crate::config::{Config, LineConfig, SegmentConfig};
use crate::segments::{SegmentContent, Style, render_sync_segment};
use crate::state::PromptState;

pub type AsyncSegmentValues = BTreeMap<String, AsyncValue>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedPrompt {
    pub line1_left: Vec<SegmentContent>,
    pub line1_right: Vec<SegmentContent>,
    pub line2_left: Vec<SegmentContent>,
    pub line2_right: Vec<SegmentContent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoweredPrompt {
    pub prompt: String,
    pub rprompt: String,
}

pub fn render(config: &Config, state: &PromptState) -> LoweredPrompt {
    lower_with_separator(
        render_structured(config, state),
        state.columns,
        config.layout.separator(),
    )
}

pub fn render_with_async(
    config: &Config,
    state: &PromptState,
    async_values: &AsyncSegmentValues,
) -> LoweredPrompt {
    lower_with_separator(
        render_structured_with_async(config, state, async_values),
        state.columns,
        config.layout.separator(),
    )
}

pub fn render_structured(config: &Config, state: &PromptState) -> RenderedPrompt {
    render_structured_inner(config, state, None)
}

pub fn render_structured_with_async(
    config: &Config,
    state: &PromptState,
    async_values: &AsyncSegmentValues,
) -> RenderedPrompt {
    render_structured_inner(config, state, Some(async_values))
}

fn render_structured_inner(
    config: &Config,
    state: &PromptState,
    async_values: Option<&AsyncSegmentValues>,
) -> RenderedPrompt {
    let line1 = render_line(&config.layout.line1, config, state, async_values);
    let line2 = if config.layout.lines == 2 {
        render_line(&config.layout.line2, config, state, async_values)
    } else {
        RenderedLine::default()
    };

    RenderedPrompt {
        line1_left: line1.left,
        line1_right: line1.right,
        line2_left: line2.left,
        line2_right: line2.right,
    }
}

pub fn lower(rendered: RenderedPrompt, columns: u16) -> LoweredPrompt {
    lower_with_separator(rendered, columns, " ")
}

fn lower_with_separator(rendered: RenderedPrompt, columns: u16, separator: &str) -> LoweredPrompt {
    let columns = usize::from(columns);
    let mut line1_left = rendered.line1_left;
    let mut line1_right = rendered.line1_right;
    let mut line2_left = rendered.line2_left;
    let mut line2_right = rendered.line2_right;

    fit_prompt_line(&mut line1_left, &mut line1_right, columns, separator);
    fit_prompt_line(&mut line2_left, &mut line2_right, columns, separator);

    if line2_left.is_empty() && line2_right.is_empty() {
        return LoweredPrompt {
            prompt: lower_last_line(&line1_left, separator),
            rprompt: lower_side(&line1_right, separator),
        };
    }

    let prompt = {
        let first_line = lower_first_line(&line1_left, &line1_right, columns, separator);
        let second_line = lower_last_line(&line2_left, separator);
        format!("{first_line}\n{second_line}")
    };

    LoweredPrompt {
        prompt,
        rprompt: lower_side(&line2_right, separator),
    }
}

fn render_line(
    line: &LineConfig,
    config: &Config,
    state: &PromptState,
    async_values: Option<&AsyncSegmentValues>,
) -> RenderedLine {
    RenderedLine {
        left: render_side(&line.left, config, state, async_values),
        right: render_side(&line.right, config, state, async_values),
    }
}

fn render_side(
    ids: &[String],
    config: &Config,
    state: &PromptState,
    async_values: Option<&AsyncSegmentValues>,
) -> Vec<SegmentContent> {
    ids.iter()
        .filter_map(|id| render_segment(id, config, state, async_values))
        .collect()
}

fn render_segment(
    id: &str,
    config: &Config,
    state: &PromptState,
    async_values: Option<&AsyncSegmentValues>,
) -> Option<SegmentContent> {
    render_sync_segment(id, state, config.segment(id)).or_else(|| {
        async_values
            .and_then(|values| values.get(id))
            .and_then(|value| async_value_content(id, value, config.segment(id)))
    })
}

fn async_value_content(
    id: &str,
    value: &AsyncValue,
    config: &SegmentConfig,
) -> Option<SegmentContent> {
    match value {
        AsyncValue::Ready(Some(content)) | AsyncValue::Stale(Some(content)) => {
            Some(content.clone())
        }
        AsyncValue::Ready(None) | AsyncValue::Stale(None) => None,
        AsyncValue::Loading => config
            .loading
            .as_ref()
            .map(|loading| SegmentContent::new(id, loading, Style::from(&config.style))),
        AsyncValue::Failed => None,
    }
}

fn lower_first_line(
    left: &[SegmentContent],
    right: &[SegmentContent],
    columns: usize,
    separator: &str,
) -> String {
    let lowered_left = lower_side(left, separator);
    let lowered_right = lower_side(right, separator);

    if right.is_empty() {
        return lowered_left;
    }

    let left_width = side_width(left, separator);
    let right_width = side_width(right, separator);
    let padding = columns.saturating_sub(left_width + right_width);
    format!("{lowered_left}{}{lowered_right}", " ".repeat(padding))
}

fn lower_last_line(left: &[SegmentContent], separator: &str) -> String {
    lower_side(left, separator)
}

fn lower_side(segments: &[SegmentContent], separator: &str) -> String {
    let separator = zsh::escape_prompt_text(separator);
    segments
        .iter()
        .map(zsh::lower_segment)
        .collect::<Vec<_>>()
        .join(&separator)
}

fn fit_prompt_line(
    left: &mut Vec<SegmentContent>,
    right: &mut Vec<SegmentContent>,
    columns: usize,
    separator: &str,
) {
    fit_side(left, columns, separator);

    if left.is_empty() {
        fit_side(right, columns, separator);
        return;
    }

    let available_for_right = columns.saturating_sub(side_width(left, separator));
    if side_width(right, separator) > available_for_right {
        right.clear();
    }
}

fn fit_side(segments: &mut Vec<SegmentContent>, columns: usize, separator: &str) {
    if side_width(segments, separator) <= columns {
        return;
    }

    shrink_dir_segment(segments, columns, separator);

    while side_width(segments, separator) > columns && segments.len() > 1 {
        segments.pop();
    }

    if side_width(segments, separator) > columns
        && let Some(segment) = segments.first_mut()
    {
        let next_text = if segment.id == "dir" {
            width::truncate_start(&segment.text, columns)
        } else {
            width::truncate_end(&segment.text, columns)
        };
        segment.text = next_text;
    }
}

fn shrink_dir_segment(segments: &mut [SegmentContent], columns: usize, separator: &str) {
    let Some(position) = segments.iter().position(|segment| segment.id == "dir") else {
        return;
    };

    let other_width = segments
        .iter()
        .enumerate()
        .filter(|(index, _segment)| *index != position)
        .map(|(_index, segment)| width::display_width(&segment.text))
        .sum::<usize>();
    let separator_width = separator_total_width(segments, separator);
    let available = columns.saturating_sub(other_width + separator_width);
    segments[position].text = width::truncate_start(&segments[position].text, available);
}

fn side_width(segments: &[SegmentContent], separator: &str) -> usize {
    if segments.is_empty() {
        return 0;
    }

    let content_width = segments
        .iter()
        .map(|segment| width::display_width(&segment.text))
        .sum::<usize>();
    content_width + separator_total_width(segments, separator)
}

fn separator_total_width(segments: &[SegmentContent], separator: &str) -> usize {
    width::display_width(separator) * segments.len().saturating_sub(1)
}

#[derive(Default)]
struct RenderedLine {
    left: Vec<SegmentContent>,
    right: Vec<SegmentContent>,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use insta::assert_snapshot;
    use proptest::prelude::*;

    use super::*;
    use crate::cache::AsyncValue;
    use crate::config::{Config, LayoutConfig, LineConfig};
    use crate::segments::Style;
    use crate::state::{Keymap, PromptEnv};

    #[test]
    fn snapshots_two_line_first_line_right_prompt() {
        let state = PromptState {
            cwd: PathBuf::from("/Users/me/projects/nova"),
            exit_status: 1,
            duration_ms: Some(2_340),
            time: Some("11:16:42".to_string()),
            columns: 32,
            keymap: Keymap::Main,
            env: PromptEnv {
                home: Some(PathBuf::from("/Users/me")),
                ..PromptEnv::default()
            },
        };
        let config = Config::default();

        assert_snapshot!(
            render(&config, &state).prompt,
            @r###"
%{[32m%}~/p/nova%{[0m%} +2.3s          11:16:42
%{[1;31m%}[1]%{[0m%} %{[1;31m%}❯ %{[0m%}
"###
        );
    }

    #[test]
    fn snapshots_one_line_with_rprompt() {
        let state = PromptState {
            cwd: PathBuf::from("/repo"),
            exit_status: 0,
            duration_ms: Some(5_000),
            time: None,
            columns: 20,
            keymap: Keymap::Main,
            env: Default::default(),
        };
        let config = Config {
            async_config: Default::default(),
            layout: LayoutConfig {
                lines: 1,
                separator: None,
                line1: LineConfig {
                    left: vec!["dir".to_string(), "prompt_char".to_string()],
                    right: vec!["duration".to_string()],
                },
                line2: LineConfig::default(),
            },
            segments: Default::default(),
        };

        let output = render(&config, &state);
        assert_snapshot!(output.prompt, @r###"%{[32m%}/repo%{[0m%} ❯ "###);
        assert_snapshot!(output.rprompt, @"+5s");
    }

    #[test]
    fn snapshots_custom_separator() {
        let state = PromptState {
            cwd: PathBuf::from("/repo"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };
        let config = Config {
            async_config: Default::default(),
            layout: LayoutConfig {
                lines: 1,
                separator: Some(" | ".to_string()),
                line1: LineConfig {
                    left: vec!["dir".to_string(), "prompt_char".to_string()],
                    right: Vec::new(),
                },
                line2: LineConfig::default(),
            },
            segments: Default::default(),
        };

        assert_snapshot!(
            render(&config, &state).prompt,
            @r###"%{[32m%}/repo%{[0m%} | ❯ "###
        );
    }

    #[test]
    fn includes_ready_async_segments_in_layout_order() {
        let state = PromptState {
            cwd: PathBuf::from("/repo"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };
        let config = Config {
            async_config: Default::default(),
            layout: LayoutConfig {
                lines: 1,
                separator: None,
                line1: LineConfig {
                    left: vec![
                        "dir".to_string(),
                        "git_branch".to_string(),
                        "git_status".to_string(),
                        "prompt_char".to_string(),
                    ],
                    right: Vec::new(),
                },
                line2: LineConfig::default(),
            },
            segments: Default::default(),
        };
        let async_values = AsyncSegmentValues::from([
            (
                "git_branch".to_string(),
                AsyncValue::Ready(Some(SegmentContent::new(
                    "git_branch",
                    "main",
                    Style::default(),
                ))),
            ),
            (
                "git_status".to_string(),
                AsyncValue::Ready(Some(SegmentContent::new(
                    "git_status",
                    "[+1]",
                    Style::default(),
                ))),
            ),
        ]);

        let rendered = render_structured_with_async(&config, &state, &async_values);

        assert_eq!(
            rendered
                .line1_left
                .iter()
                .map(|segment| segment.id.as_str())
                .collect::<Vec<_>>(),
            ["dir", "git_branch", "git_status", "prompt_char"]
        );
        assert_eq!(rendered.line1_left[1].text, "main");
        assert_eq!(rendered.line1_left[2].text, "[+1]");
    }

    #[test]
    fn includes_stale_async_segments_and_omits_unavailable_states() {
        let state = PromptState {
            cwd: PathBuf::from("/repo"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };
        let config = Config {
            async_config: Default::default(),
            layout: LayoutConfig {
                lines: 1,
                separator: None,
                line1: LineConfig {
                    left: vec![
                        "git_branch".to_string(),
                        "git_status".to_string(),
                        "bun_version".to_string(),
                        "node_version".to_string(),
                        "runtime".to_string(),
                        "prompt_char".to_string(),
                    ],
                    right: Vec::new(),
                },
                line2: LineConfig::default(),
            },
            segments: Default::default(),
        };
        let async_values = AsyncSegmentValues::from([
            (
                "git_branch".to_string(),
                AsyncValue::Stale(Some(SegmentContent::new(
                    "git_branch",
                    "main",
                    Style::default(),
                ))),
            ),
            ("git_status".to_string(), AsyncValue::Loading),
            ("bun_version".to_string(), AsyncValue::Stale(None)),
            ("node_version".to_string(), AsyncValue::Ready(None)),
            ("runtime".to_string(), AsyncValue::Failed),
        ]);

        let rendered = render_structured_with_async(&config, &state, &async_values);

        assert_eq!(
            rendered
                .line1_left
                .iter()
                .map(|segment| segment.id.as_str())
                .collect::<Vec<_>>(),
            ["git_branch", "prompt_char"]
        );
        assert_eq!(rendered.line1_left[0].text, "main");
    }

    #[test]
    fn renders_configured_loading_placeholder() {
        let state = PromptState {
            cwd: PathBuf::from("/repo"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };
        let config = Config::from_toml(
            r#"
            [layout]
            lines = 1

            [layout.line1]
            left = ["git_status", "prompt_char"]
            right = []

            [segments.git_status]
            loading = "…"
            style = { fg = "yellow", bold = true }
            "#,
        )
        .expect("config should parse");
        let async_values =
            AsyncSegmentValues::from([("git_status".to_string(), AsyncValue::Loading)]);

        assert_snapshot!(
            render_with_async(&config, &state, &async_values).prompt,
            @r###"%{[1;33m%}…%{[0m%} ❯ "###
        );
    }

    #[test]
    fn escapes_percent_in_dynamic_text() {
        let state = PromptState {
            cwd: PathBuf::from("/tmp/100%real"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: Default::default(),
        };

        assert!(
            render(&Config::default(), &state)
                .prompt
                .contains("100%%real")
        );
    }

    #[test]
    fn renders_nix_shell_sync_segment() {
        let state = PromptState {
            cwd: PathBuf::from("/repo"),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 80,
            keymap: Keymap::Main,
            env: PromptEnv {
                in_nix_shell: Some("pure".to_string()),
                ..PromptEnv::default()
            },
        };
        let config = Config {
            async_config: Default::default(),
            layout: LayoutConfig {
                lines: 1,
                separator: None,
                line1: LineConfig {
                    left: vec!["nix_shell".to_string()],
                    right: Vec::new(),
                },
                line2: LineConfig::default(),
            },
            segments: Default::default(),
        };

        assert!(render(&config, &state).prompt.contains(" pure"));
    }

    proptest! {
        #[test]
        fn first_line_never_exceeds_columns(path in "\\PC{0,80}", columns in 1_u16..120) {
            let state = PromptState {
                cwd: PathBuf::from(format!("/{path}")),
                exit_status: 0,
                duration_ms: Some(10_000),
                time: None,
                columns,
                keymap: Keymap::Main,
                env: Default::default(),
            };
            let output = render(&Config::default(), &state);
            let first_line = output.prompt.lines().next().unwrap_or_default();

            prop_assert!(visible_prompt_width(first_line) <= usize::from(columns));
        }

        #[test]
        fn first_line_never_exceeds_columns_with_custom_separator(
            path in "\\PC{0,80}",
            separator in "\\PC{0,4}",
            columns in 1_u16..120,
        ) {
            let state = PromptState {
                cwd: PathBuf::from(format!("/{path}")),
                exit_status: 0,
                duration_ms: Some(10_000),
                time: None,
                columns,
                keymap: Keymap::Main,
                env: Default::default(),
            };
            let config = Config {
                async_config: Default::default(),
                layout: LayoutConfig {
                    lines: 1,
                    separator: Some(separator),
                    line1: LineConfig {
                        left: vec![
                            "dir".to_string(),
                            "git_branch".to_string(),
                            "duration".to_string(),
                            "prompt_char".to_string(),
                        ],
                        right: Vec::new(),
                    },
                    line2: LineConfig::default(),
                },
                segments: Default::default(),
            };
            let output = render(&config, &state);
            let first_line = output.prompt.lines().next().unwrap_or_default();

            prop_assert!(visible_prompt_width(first_line) <= usize::from(columns));
        }
    }

    fn visible_prompt_width(input: &str) -> usize {
        let mut output = String::new();
        let mut chars = input.chars().peekable();

        while let Some(character) = chars.next() {
            if character == '%' && chars.peek() == Some(&'{') {
                chars.next();
                while let Some(next) = chars.next() {
                    if next == '%' && chars.peek() == Some(&'}') {
                        chars.next();
                        break;
                    }
                }
            } else {
                output.push(character);
            }
        }

        width::display_width(&output.replace("%%", "%"))
    }
}
