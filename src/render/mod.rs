//! Prompt composition and lowering.

pub mod width;
pub mod zsh;

use crate::config::{Config, LineConfig};
use crate::segments::{SegmentContent, render_sync_segment};
use crate::state::PromptState;

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
    lower(render_structured(config, state), state.columns)
}

pub fn render_structured(config: &Config, state: &PromptState) -> RenderedPrompt {
    let line1 = render_line(&config.layout.line1, config, state);
    let line2 = if config.layout.lines == 2 {
        render_line(&config.layout.line2, config, state)
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
    let columns = usize::from(columns);
    let mut line1_left = rendered.line1_left;
    let mut line1_right = rendered.line1_right;
    let mut line2_left = rendered.line2_left;
    let mut line2_right = rendered.line2_right;

    fit_prompt_line(&mut line1_left, &mut line1_right, columns);
    fit_prompt_line(&mut line2_left, &mut line2_right, columns);

    if line2_left.is_empty() && line2_right.is_empty() {
        return LoweredPrompt {
            prompt: lower_last_line(&line1_left),
            rprompt: lower_side(&line1_right),
        };
    }

    let prompt = {
        let first_line = lower_first_line(&line1_left, &line1_right, columns);
        let second_line = lower_last_line(&line2_left);
        format!("{first_line}\n{second_line}")
    };

    LoweredPrompt {
        prompt,
        rprompt: lower_side(&line2_right),
    }
}

fn render_line(line: &LineConfig, config: &Config, state: &PromptState) -> RenderedLine {
    RenderedLine {
        left: render_side(&line.left, config, state),
        right: render_side(&line.right, config, state),
    }
}

fn render_side(ids: &[String], config: &Config, state: &PromptState) -> Vec<SegmentContent> {
    ids.iter()
        .filter_map(|id| render_sync_segment(id, state, &config.segment(id)))
        .collect()
}

fn lower_first_line(left: &[SegmentContent], right: &[SegmentContent], columns: usize) -> String {
    let lowered_left = lower_side(left);
    let lowered_right = lower_side(right);

    if right.is_empty() {
        return lowered_left;
    }

    let left_width = side_width(left);
    let right_width = side_width(right);
    let padding = columns.saturating_sub(left_width + right_width);
    format!("{lowered_left}{}{lowered_right}", " ".repeat(padding))
}

fn lower_last_line(left: &[SegmentContent]) -> String {
    lower_side(left)
}

fn lower_side(segments: &[SegmentContent]) -> String {
    segments
        .iter()
        .map(zsh::lower_segment)
        .collect::<Vec<_>>()
        .join(" ")
}

fn fit_prompt_line(
    left: &mut Vec<SegmentContent>,
    right: &mut Vec<SegmentContent>,
    columns: usize,
) {
    fit_side(left, columns);

    if left.is_empty() {
        fit_side(right, columns);
        return;
    }

    let available_for_right = columns.saturating_sub(side_width(left));
    if side_width(right) > available_for_right {
        right.clear();
    }
}

fn fit_side(segments: &mut Vec<SegmentContent>, columns: usize) {
    if side_width(segments) <= columns {
        return;
    }

    shrink_dir_segment(segments, columns);

    while side_width(segments) > columns && segments.len() > 1 {
        segments.pop();
    }

    if side_width(segments) > columns
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

fn shrink_dir_segment(segments: &mut [SegmentContent], columns: usize) {
    let Some(position) = segments.iter().position(|segment| segment.id == "dir") else {
        return;
    };

    let other_width = segments
        .iter()
        .enumerate()
        .filter(|(index, _segment)| *index != position)
        .map(|(_index, segment)| width::display_width(&segment.text))
        .sum::<usize>();
    let separator_width = segments.len().saturating_sub(1);
    let available = columns.saturating_sub(other_width + separator_width);
    segments[position].text = width::truncate_start(&segments[position].text, available);
}

fn side_width(segments: &[SegmentContent]) -> usize {
    if segments.is_empty() {
        return 0;
    }

    let content_width = segments
        .iter()
        .map(|segment| width::display_width(&segment.text))
        .sum::<usize>();
    content_width + segments.len().saturating_sub(1)
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
    use crate::config::{Config, LayoutConfig, LineConfig};
    use crate::state::Keymap;

    #[test]
    fn snapshots_two_line_first_line_right_prompt() {
        let state = PromptState {
            cwd: PathBuf::from("/Users/me/projects/nova"),
            exit_status: 1,
            duration_ms: Some(2_340),
            columns: 32,
            keymap: Keymap::Main,
        };
        let config = Config::default();

        assert_snapshot!(
            render(&config, &state).prompt,
            @r###"
/Users/me/projects/nova     2.3s
%{[1;31m%}❯%{[0m%}
"###
        );
    }

    #[test]
    fn snapshots_one_line_with_rprompt() {
        let state = PromptState {
            cwd: PathBuf::from("/repo"),
            exit_status: 0,
            duration_ms: Some(5_000),
            columns: 20,
            keymap: Keymap::Main,
        };
        let config = Config {
            layout: LayoutConfig {
                lines: 1,
                line1: LineConfig {
                    left: vec!["dir".to_string(), "prompt_char".to_string()],
                    right: vec!["duration".to_string()],
                },
                line2: LineConfig::default(),
            },
            segments: Default::default(),
        };

        let output = render(&config, &state);
        assert_snapshot!(output.prompt, @r###"/repo %{[1;32m%}❯%{[0m%}"###);
        assert_snapshot!(output.rprompt, @"5.0s");
    }

    #[test]
    fn escapes_percent_in_dynamic_text() {
        let state = PromptState {
            cwd: PathBuf::from("/tmp/100%real"),
            exit_status: 0,
            duration_ms: None,
            columns: 80,
            keymap: Keymap::Main,
        };

        assert!(
            render(&Config::default(), &state)
                .prompt
                .contains("100%%real")
        );
    }

    proptest! {
        #[test]
        fn first_line_never_exceeds_columns(path in "\\PC{0,80}", columns in 1_u16..120) {
            let state = PromptState {
                cwd: PathBuf::from(format!("/{path}")),
                exit_status: 0,
                duration_ms: Some(10_000),
                columns,
                keymap: Keymap::Main,
            };
            let output = render(&Config::default(), &state);
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
