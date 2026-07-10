//! Prompt composition and lowering.

pub mod width;
pub mod zsh;

use std::collections::BTreeMap;

use crate::cache::AsyncValue;
use crate::config::{Config, LineConfig, SegmentConfig};
use crate::segments::{SegmentContent, SegmentPart, Style, render_sync_segment};
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

    fit_prompt_line(&mut line2_left, &mut line2_right, columns, separator);

    if line2_left.is_empty() && line2_right.is_empty() {
        fit_prompt_line(&mut line1_left, &mut line1_right, columns, separator);
        let input_line = lower_input_line(&line1_left, columns, separator);
        let rprompt =
            lower_input_rprompt(&input_line, &line1_left, &line1_right, columns, separator);
        return LoweredPrompt {
            prompt: input_line.prompt,
            rprompt,
        };
    }

    fit_right_side(&mut line1_right, columns, separator);
    let first_line = lower_first_line(&mut line1_left, &line1_right, columns, separator);
    let second_line = lower_input_line(&line2_left, columns, separator);
    let prompt = format!("{first_line}\n{}", second_line.prompt);
    let rprompt = lower_input_rprompt(&second_line, &line2_left, &line2_right, columns, separator);

    LoweredPrompt { prompt, rprompt }
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
    left: &mut Vec<SegmentContent>,
    right: &[SegmentContent],
    columns: usize,
    separator: &str,
) -> String {
    if right.is_empty() || columns < MIN_LINE_RIGHT_COLUMNS {
        fit_side(left, columns, separator);
        return lower_truncated_start_side(left, columns, separator);
    }

    let lowered_right = lower_side(right, separator);
    let right_width = side_width(right, separator);
    let available_for_left = columns.saturating_sub(right_width + MIN_LINE_RIGHT_GAP);
    fit_side(left, available_for_left, separator);
    let lowered_left = lower_truncated_start_side(left, available_for_left, separator);
    let padding = columns.saturating_sub(visible_prompt_width(&lowered_left) + right_width);
    format!("{lowered_left}{}{lowered_right}", " ".repeat(padding))
}

const MIN_LINE_RIGHT_GAP: usize = 1;
const MIN_LINE_RIGHT_COLUMNS: usize = 50;

struct LoweredInputLine {
    prompt: String,
    wrapped: bool,
}

fn lower_input_line(left: &[SegmentContent], columns: usize, separator: &str) -> LoweredInputLine {
    let prompt_columns = prompt_columns_for_input(columns);
    if side_width(left, separator) <= prompt_columns {
        return LoweredInputLine {
            prompt: lower_side(left, separator),
            wrapped: false,
        };
    }

    let Some(prompt_char_index) = left.iter().rposition(|segment| segment.id == "prompt_char")
    else {
        return LoweredInputLine {
            prompt: lower_truncated_start_side(left, prompt_columns, separator),
            wrapped: true,
        };
    };

    let info = left
        .iter()
        .enumerate()
        .filter(|(index, _segment)| *index != prompt_char_index)
        .map(|(_index, segment)| segment.clone())
        .collect::<Vec<_>>();
    let prompt_char = [left[prompt_char_index].clone()];
    let prompt_char_line = lower_side(&prompt_char, separator);

    if info.is_empty() {
        return LoweredInputLine {
            prompt: prompt_char_line,
            wrapped: false,
        };
    }

    let info_line = lower_truncated_start_side(&info, columns, separator);
    LoweredInputLine {
        prompt: format!("{info_line}\n{prompt_char_line}"),
        wrapped: true,
    }
}

fn lower_input_rprompt(
    input_line: &LoweredInputLine,
    left: &[SegmentContent],
    right: &[SegmentContent],
    columns: usize,
    separator: &str,
) -> String {
    if input_line.wrapped || right.is_empty() {
        return String::new();
    }

    let lowered_right = lower_truncated_start_side(right, columns, separator);
    let right_width = visible_prompt_width(&lowered_right);
    if right_width == 0 || side_width(left, separator) + right_width > columns {
        String::new()
    } else {
        lowered_right
    }
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
    fit_right_side(right, columns, separator);
}

fn fit_side(segments: &mut Vec<SegmentContent>, columns: usize, separator: &str) {
    fit_aligned_side(segments, columns, separator, SideAlignment::Left);
}

fn fit_right_side(segments: &mut Vec<SegmentContent>, columns: usize, separator: &str) {
    fit_aligned_side(segments, columns, separator, SideAlignment::Right);
}

#[derive(Clone, Copy)]
enum SideAlignment {
    Left,
    Right,
}

fn fit_aligned_side(
    segments: &mut Vec<SegmentContent>,
    columns: usize,
    separator: &str,
    alignment: SideAlignment,
) {
    remove_zero_width_segments(segments);

    if side_width(segments, separator) <= columns {
        return;
    }

    while side_width(segments, separator) > columns {
        if strip_git_status_counts(segments)
            || compact_icon_segments(segments)
            || drop_user_host_segment(segments)
            || drop_secondary_segments(segments)
            || compact_dir_segment(segments)
            || shrink_branch_segment(segments, columns, separator, BRANCH_SOFT_MIN_WIDTH)
            || shrink_dir_segment_preserving_floor(segments, columns, separator)
        {
            remove_zero_width_segments(segments);
            continue;
        }

        match alignment {
            SideAlignment::Right => {
                *segments = truncate_start_side(segments, columns, separator);
                break;
            }
            SideAlignment::Left if segments.iter().any(|segment| segment.id == "prompt_char") => {
                truncate_first_segment(segments, columns);
                remove_zero_width_segments(segments);
                break;
            }
            SideAlignment::Left => {
                if !truncate_widest_segment_to_fit(segments, columns, separator) {
                    break;
                }
                remove_zero_width_segments(segments);
            }
        }
    }
}

fn truncate_first_segment(segments: &mut [SegmentContent], columns: usize) {
    let Some(segment) = segments.first_mut() else {
        return;
    };

    match segment.id.as_str() {
        "dir" => set_segment_text(segment, truncate_dir_text(&segment.text, columns)),
        "git_branch" => {
            set_segment_text(segment, truncate_branch_text(&segment.text, columns));
        }
        "prompt_char" => {
            set_segment_text(segment, truncate_prompt_char_text(&segment.text, columns));
        }
        _ => truncate_segment_end(segment, columns),
    }
}

fn truncate_widest_segment_to_fit(
    segments: &mut [SegmentContent],
    columns: usize,
    separator: &str,
) -> bool {
    let current_side_width = side_width(segments, separator);
    if current_side_width <= columns {
        return false;
    }

    let position = segments
        .iter()
        .enumerate()
        .filter(|(_index, segment)| segment.id != "prompt_char")
        .max_by_key(|(_index, segment)| width::display_width(&segment.text))
        .or_else(|| {
            segments
                .iter()
                .enumerate()
                .max_by_key(|(_index, segment)| width::display_width(&segment.text))
        })
        .map(|(index, _segment)| index);
    let Some(position) = position else {
        return false;
    };

    let current_width = width::display_width(&segments[position].text);
    let overflow = current_side_width - columns;
    let target_width = current_width.saturating_sub(overflow);
    let segment = &mut segments[position];

    match segment.id.as_str() {
        "dir" => set_segment_text(segment, truncate_dir_text(&segment.text, target_width)),
        "git_branch" => {
            set_segment_text(segment, truncate_branch_text(&segment.text, target_width));
        }
        "prompt_char" => {
            set_segment_text(
                segment,
                truncate_prompt_char_text(&segment.text, target_width),
            );
        }
        _ => truncate_segment_end(segment, target_width),
    }

    width::display_width(&segment.text) < current_width
}

const BRANCH_SOFT_MIN_WIDTH: usize = 10;
const MIN_COMMAND_COLUMNS: usize = 20;
const MAX_COMMAND_COLUMNS: usize = 60;
const COMMAND_COLUMNS_PCT: usize = 50;
const MIN_INPUT_PROMPT_COLUMNS: usize = 2;

fn command_columns(columns: usize) -> usize {
    let desired = MIN_COMMAND_COLUMNS.max(columns * COMMAND_COLUMNS_PCT / 100);
    let max_reserved = columns.saturating_sub(MIN_INPUT_PROMPT_COLUMNS);
    desired.min(MAX_COMMAND_COLUMNS).min(max_reserved)
}

fn prompt_columns_for_input(columns: usize) -> usize {
    columns.saturating_sub(command_columns(columns))
}

fn compact_icon_segments(segments: &mut [SegmentContent]) -> bool {
    let mut compacted = false;

    for segment in segments
        .iter_mut()
        .filter(|segment| can_compact_to_icon(&segment.id))
    {
        let Some(icon) = icon_only_text(&segment.text) else {
            continue;
        };

        if width::display_width(&icon) >= width::display_width(&segment.text) {
            continue;
        }

        set_segment_text(segment, icon);
        compacted = true;
    }

    compacted
}

fn drop_secondary_segments(segments: &mut Vec<SegmentContent>) -> bool {
    let initial_len = segments.len();
    segments.retain(|segment| !is_secondary_segment(&segment.id));
    segments.len() != initial_len
}

fn drop_user_host_segment(segments: &mut Vec<SegmentContent>) -> bool {
    if !segments
        .iter()
        .any(|segment| is_primary_segment(&segment.id))
    {
        return false;
    }

    let Some(position) = segments
        .iter()
        .position(|segment| segment.id == "user_host")
    else {
        return false;
    };

    segments.remove(position);
    true
}

fn is_primary_segment(id: &str) -> bool {
    matches!(id, "dir" | "git_branch" | "git_status")
}

fn is_secondary_segment(id: &str) -> bool {
    can_compact_to_icon(id) || id == "duration"
}

fn can_compact_to_icon(id: &str) -> bool {
    matches!(
        id,
        "rust_version"
            | "bun_version"
            | "deno_version"
            | "node_version"
            | "python_version"
            | "nix_shell"
            | "aws"
    )
}

fn icon_only_text(text: &str) -> Option<String> {
    let icon = text.split_whitespace().next()?;
    if icon == text {
        None
    } else {
        Some(icon.to_string())
    }
}

fn compact_dir_segment(segments: &mut [SegmentContent]) -> bool {
    let Some(segment) = segments.iter_mut().find(|segment| segment.id == "dir") else {
        return false;
    };

    let Some(text) = compact_dir_path_text(&segment.text) else {
        return false;
    };

    if width::display_width(&text) >= width::display_width(&segment.text) {
        return false;
    }

    set_segment_text(segment, text);
    true
}

fn shrink_branch_segment(
    segments: &mut [SegmentContent],
    columns: usize,
    separator: &str,
    min_width: usize,
) -> bool {
    shrink_segment_to_fit(
        segments,
        "git_branch",
        columns,
        separator,
        min_width,
        truncate_branch_text,
    )
}

fn shrink_dir_segment_preserving_floor(
    segments: &mut [SegmentContent],
    columns: usize,
    separator: &str,
) -> bool {
    let min_width = segments
        .iter()
        .find(|segment| segment.id == "dir")
        .map(|segment| width::display_width(dir_floor_text(&segment.text)))
        .unwrap_or(1)
        .max(1);

    shrink_dir_segment(segments, columns, separator, min_width)
}

fn shrink_dir_segment(
    segments: &mut [SegmentContent],
    columns: usize,
    separator: &str,
    min_width: usize,
) -> bool {
    shrink_segment_to_fit(
        segments,
        "dir",
        columns,
        separator,
        min_width,
        truncate_dir_text,
    )
}

fn shrink_segment_to_fit(
    segments: &mut [SegmentContent],
    id: &str,
    columns: usize,
    separator: &str,
    min_width: usize,
    truncate: impl Fn(&str, usize) -> String,
) -> bool {
    let current_side_width = side_width(segments, separator);
    if current_side_width <= columns {
        return false;
    }

    let Some(position) = segments.iter().position(|segment| segment.id == id) else {
        return false;
    };

    let current_width = width::display_width(&segments[position].text);
    if current_width == 0 {
        return false;
    }

    let overflow = current_side_width.saturating_sub(columns);
    let target_width = current_width.saturating_sub(overflow).max(min_width);
    if target_width >= current_width {
        return false;
    }

    let next_text = truncate(&segments[position].text, target_width);
    if width::display_width(&next_text) >= current_width {
        return false;
    }

    set_segment_text(&mut segments[position], next_text);
    true
}

fn dir_floor_text(text: &str) -> &str {
    text.split('/')
        .rev()
        .find(|component| !component.is_empty())
        .unwrap_or(text)
}

fn truncate_dir_text(text: &str, max_width: usize) -> String {
    if width::display_width(text) <= max_width {
        return text.to_string();
    }

    let compact_text = compact_dir_path_text(text);
    let candidate = compact_text.as_deref().unwrap_or(text);
    if width::display_width(candidate) <= max_width {
        return candidate.to_string();
    }

    let floor = dir_floor_text(candidate);
    let floor_width = width::display_width(floor);
    if max_width == floor_width {
        return floor.to_string();
    }

    if max_width < floor_width {
        return width::truncate_start(floor, max_width);
    }

    width::truncate_start(candidate, max_width)
}

fn compact_dir_path_text(text: &str) -> Option<String> {
    let components = text.split('/').collect::<Vec<_>>();
    if components.len() <= 1 {
        return None;
    }

    let last_index = components.len().saturating_sub(1);
    let compacted = components
        .iter()
        .enumerate()
        .map(|(index, component)| {
            if should_preserve_dir_component(component, index, last_index) {
                (*component).to_string()
            } else {
                abbreviate_dir_component(component)
            }
        })
        .collect::<Vec<_>>()
        .join("/");

    if compacted == text {
        None
    } else {
        Some(compacted)
    }
}

fn should_preserve_dir_component(component: &str, index: usize, last_index: usize) -> bool {
    index == last_index || component.is_empty() || matches!(component, "~" | "…")
}

fn abbreviate_dir_component(component: &str) -> String {
    let mut chars = component.chars();
    match chars.next() {
        Some('.') => chars
            .next()
            .map(|next| format!(".{next}"))
            .unwrap_or_else(|| ".".to_string()),
        Some(first) => first.to_string(),
        None => String::new(),
    }
}

fn truncate_branch_text(text: &str, max_width: usize) -> String {
    if width::display_width(text) <= max_width {
        return text.to_string();
    }

    let Some((prefix, label)) = split_label_prefix(text) else {
        return width::truncate_middle(text, max_width);
    };

    let prefix_width = width::display_width(prefix);
    if max_width <= prefix_width {
        return width::truncate_middle(text, max_width);
    }

    let label_width = max_width - prefix_width;
    let label = width::truncate_middle(label, label_width);
    format!("{prefix}{label}")
}

fn truncate_prompt_char_text(text: &str, max_width: usize) -> String {
    if width::display_width(text) <= max_width {
        return text.to_string();
    }

    let trimmed = text.trim_end();
    if width::display_width(trimmed) <= max_width {
        trimmed.to_string()
    } else {
        width::truncate_end(text, max_width)
    }
}

fn split_label_prefix(text: &str) -> Option<(&str, &str)> {
    let separator_start = text
        .char_indices()
        .find(|(_index, character)| character.is_whitespace())
        .map(|(index, _character)| index)?;
    let label_start = text[separator_start..]
        .char_indices()
        .find(|(_index, character)| !character.is_whitespace())
        .map(|(index, _character)| separator_start + index)?;

    Some((&text[..label_start], &text[label_start..]))
}

fn strip_git_status_counts(segments: &mut [SegmentContent]) -> bool {
    let Some(segment) = segments
        .iter_mut()
        .find(|segment| segment.id == "git_status")
    else {
        return false;
    };

    let text = segment
        .text
        .chars()
        .filter(|character| !character.is_ascii_digit())
        .collect::<String>();

    if text == segment.text || width::display_width(&text) >= width::display_width(&segment.text) {
        return false;
    }

    set_segment_text(segment, text);
    true
}

fn remove_zero_width_segments(segments: &mut Vec<SegmentContent>) {
    segments.retain(|segment| width::display_width(&segment.text) > 0);
}

fn set_segment_text(segment: &mut SegmentContent, text: String) {
    if segment.uses_parts()
        && let Some(part) = segment.parts.first()
    {
        segment.style = part.style.clone();
    }

    segment.text = text;
    segment.parts.clear();
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

fn lower_truncated_start_side(
    segments: &[SegmentContent],
    max_width: usize,
    separator: &str,
) -> String {
    lower_side(
        &truncate_start_side(segments, max_width, separator),
        separator,
    )
}

fn truncate_start_side(
    segments: &[SegmentContent],
    max_width: usize,
    separator: &str,
) -> Vec<SegmentContent> {
    if max_width == 0 || segments.is_empty() {
        return Vec::new();
    }

    if side_width(segments, separator) <= max_width {
        return segments.to_vec();
    }

    let separator_width = width::display_width(separator);
    let mut used_width = 0;
    let mut kept = Vec::new();

    for segment in segments.iter().rev() {
        let separator_before = if kept.is_empty() { 0 } else { separator_width };
        let available = max_width.saturating_sub(used_width + separator_before);
        if available == 0 {
            break;
        }

        let segment_width = width::display_width(&segment.text);
        if segment_width <= available {
            kept.push(segment.clone());
            used_width += separator_before + segment_width;
            continue;
        }

        let segment = truncate_segment_start(segment, available);
        if !segment.text.is_empty() {
            kept.push(segment);
        }
        break;
    }

    kept.reverse();
    force_leading_ellipsis(&mut kept);
    kept
}

fn force_leading_ellipsis(segments: &mut [SegmentContent]) {
    let Some(first) = segments.first_mut() else {
        return;
    };

    if first.text.starts_with('…') {
        return;
    }

    let first_width = width::display_width(&first.text);
    if first_width <= 1 {
        let style = first
            .parts
            .first()
            .map(|part| part.style.clone())
            .unwrap_or_else(|| first.style.clone());
        *first = SegmentContent::new(first.id.clone(), "…", style);
        return;
    }

    *first = truncate_segment_start(first, first_width.saturating_sub(1));
}

fn truncate_segment_end(segment: &mut SegmentContent, max_width: usize) {
    *segment = truncate_segment(segment, max_width, TruncateSide::End);
}

fn truncate_segment_start(segment: &SegmentContent, max_width: usize) -> SegmentContent {
    truncate_segment(segment, max_width, TruncateSide::Start)
}

fn truncate_segment(
    segment: &SegmentContent,
    max_width: usize,
    side: TruncateSide,
) -> SegmentContent {
    if width::display_width(&segment.text) <= max_width {
        return segment.clone();
    }

    if !segment.uses_parts() {
        let text = match side {
            TruncateSide::Start => width::truncate_start(&segment.text, max_width),
            TruncateSide::End => width::truncate_end(&segment.text, max_width),
        };
        let mut segment = segment.clone();
        set_segment_text(&mut segment, text);
        return segment;
    }

    let ellipsis_style = match side {
        TruncateSide::Start => segment
            .parts
            .last()
            .map(|part| part.style.clone())
            .unwrap_or_else(|| segment.style.clone()),
        TruncateSide::End => segment
            .parts
            .first()
            .map(|part| part.style.clone())
            .unwrap_or_else(|| segment.style.clone()),
    };

    if max_width == 0 {
        return SegmentContent::new(segment.id.clone(), String::new(), ellipsis_style);
    }

    let content_width = max_width - 1;
    let mut parts = match side {
        TruncateSide::Start => retained_suffix_parts(&segment.parts, content_width),
        TruncateSide::End => retained_prefix_parts(&segment.parts, content_width),
    };

    match side {
        TruncateSide::Start => {
            if let Some(first) = parts.first_mut() {
                first.text.insert(0, '…');
            } else {
                parts.push(SegmentPart::new("…", ellipsis_style));
            }
        }
        TruncateSide::End => {
            if let Some(last) = parts.last_mut() {
                last.text.push('…');
            } else {
                parts.push(SegmentPart::new("…", ellipsis_style));
            }
        }
    }

    SegmentContent::from_parts(segment.id.clone(), parts)
}

fn retained_prefix_parts(parts: &[SegmentPart], max_width: usize) -> Vec<SegmentPart> {
    let mut remaining = max_width;
    let mut retained = Vec::new();

    for part in parts {
        let part_width = width::display_width(&part.text);
        if part_width <= remaining {
            retained.push(part.clone());
            remaining -= part_width;
            continue;
        }

        let text = width::take_prefix(&part.text, remaining);
        if !text.is_empty() {
            retained.push(SegmentPart::new(text, part.style.clone()));
        }
        break;
    }

    retained
}

fn retained_suffix_parts(parts: &[SegmentPart], max_width: usize) -> Vec<SegmentPart> {
    let mut remaining = max_width;
    let mut retained = Vec::new();

    for part in parts.iter().rev() {
        let part_width = width::display_width(&part.text);
        if part_width <= remaining {
            retained.push(part.clone());
            remaining -= part_width;
            continue;
        }

        let text = width::take_suffix(&part.text, remaining);
        if !text.is_empty() {
            retained.push(SegmentPart::new(text, part.style.clone()));
        }
        break;
    }

    retained.reverse();
    retained
}

#[derive(Clone, Copy)]
enum TruncateSide {
    Start,
    End,
}

fn strip_prompt_markers(input: &str) -> String {
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
        } else if character == '%' && chars.peek() == Some(&'%') {
            chars.next();
            output.push('%');
        } else {
            output.push(character);
        }
    }

    output
}

fn visible_prompt_width(input: &str) -> usize {
    width::display_width(&strip_prompt_markers(input))
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
    use crate::segments::{SegmentPart, Style};
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
%{[32m%}~/p/nova%{[0m%} +2.3s
%{[1;31m%}[1]%{[0m%} %{[1;31m%}❯ %{[0m%}
"###
        );
    }

    #[test]
    fn two_line_right_prompt_reserves_a_gap_at_minimum_width() {
        let long_left = "a".repeat(50);
        let output = lower(
            RenderedPrompt {
                line1_left: vec![test_segment("custom", &long_left)],
                line1_right: vec![test_segment("time", "12:34:56")],
                line2_left: vec![test_segment("prompt_char", ">_")],
                line2_right: Vec::new(),
            },
            50,
        );

        let first_line = output.prompt.lines().next().expect("first prompt line");
        assert_eq!(visible_prompt_width(first_line), 50);
        assert!(first_line.ends_with(" 12:34:56"));
    }

    #[test]
    fn two_line_right_prompt_is_hidden_below_minimum_width() {
        let output = lower(
            RenderedPrompt {
                line1_left: vec![test_segment("custom", "left")],
                line1_right: vec![test_segment("time", "12:34:56")],
                line2_left: vec![test_segment("prompt_char", ">_")],
                line2_right: Vec::new(),
            },
            49,
        );

        assert_eq!(output.prompt, "left\n>_");
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
        assert_eq!(output.prompt, "%{\x1b[32m%}/repo%{\x1b[0m%}\n❯ ");
        assert_snapshot!(output.rprompt, @"");
    }

    #[test]
    fn input_prompt_reserves_command_columns() {
        assert_eq!(command_columns(200), 60);
        assert_eq!(command_columns(160), 60);
        assert_eq!(command_columns(120), 60);
        assert_eq!(command_columns(80), 40);
        assert_eq!(command_columns(50), 25);
        assert_eq!(command_columns(20), 18);

        assert_eq!(prompt_columns_for_input(200), 140);
        assert_eq!(prompt_columns_for_input(160), 100);
        assert_eq!(prompt_columns_for_input(120), 60);
        assert_eq!(prompt_columns_for_input(80), 40);
        assert_eq!(prompt_columns_for_input(50), 25);
        assert_eq!(prompt_columns_for_input(20), 2);
    }

    #[test]
    fn input_line_wraps_prompt_char_when_prompt_exceeds_command_budget() {
        let mut left = vec![
            test_segment("dir", "~/dev/oss/nova-example/src/render"),
            test_segment(
                "git_branch",
                "feature/render-fitting-priority-with-a-very-long-suffix-check",
            ),
            test_segment("git_status", "[!1+1?1]"),
            test_segment("rust_version", " 1.96.1"),
            test_segment("nix_shell", " impure (nix-shell-env)"),
            test_segment("aws", " very-long-aws-profile (ap-northeast-1)"),
            test_segment("prompt_char", "❯ "),
        ];
        let mut right = vec![test_segment("time", "22:50:54")];

        fit_prompt_line(&mut left, &mut right, 80, " ");
        let input_line = lower_input_line(&left, 80, " ");

        assert!(input_line.wrapped);
        assert!(input_line.prompt.ends_with("\n❯ "));
    }

    #[test]
    fn input_line_drops_user_host_before_wrapped_prompt_char() {
        let mut left = vec![
            test_segment("user_host", "t1190078@M4Pro"),
            test_segment("dir", "~/dev/oss/nova"),
            test_segment("git_branch", "(fix/32-render-fitting-priority)"),
            test_segment("prompt_char", "❯ "),
        ];
        let mut right = Vec::new();

        fit_prompt_line(&mut left, &mut right, 25, " ");
        let input_line = lower_input_line(&left, 25, " ");
        let lines = input_line.prompt.lines().collect::<Vec<_>>();

        assert!(input_line.wrapped);
        assert!(!lines[0].contains("t1190078@M4Pro"));
        assert!(lines[0].contains("fix"));
        assert_eq!(lines[1], "❯ ");
    }

    #[test]
    fn input_rprompt_is_hidden_when_it_would_collide() {
        let output = lower(
            RenderedPrompt {
                line1_left: vec![test_segment("custom", "left-prompt-that-still-fits-input")],
                line1_right: vec![test_segment(
                    "custom",
                    "right-prompt-that-would-overlap-the-input-prompt",
                )],
                line2_left: Vec::new(),
                line2_right: Vec::new(),
            },
            80,
        );

        assert_eq!(output.prompt, "left-prompt-that-still-fits-input");
        assert_eq!(output.rprompt, "");
    }

    #[test]
    fn oversized_input_rprompt_is_truncated_to_columns() {
        let output = lower(
            RenderedPrompt {
                line1_left: Vec::new(),
                line1_right: vec![
                    test_segment("time", "12:34:56"),
                    test_segment(
                        "custom",
                        "right-prompt-that-is-longer-than-the-entire-terminal-width",
                    ),
                ],
                line2_left: Vec::new(),
                line2_right: Vec::new(),
            },
            32,
        );

        assert!(visible_prompt_width(&output.rprompt) <= 32);
        assert!(output.rprompt.starts_with('…'));
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

    #[test]
    fn narrow_fitting_prioritizes_dir_and_branch_over_user_host() {
        let mut segments = vec![
            test_segment("user_host", "user@host"),
            test_segment("dir", "~/dev/oss/nova"),
            test_segment("git_branch", "feature/some-branch"),
            test_segment("node_version", "24.16.0"),
        ];

        fit_side(&mut segments, 14, " ");

        assert_eq!(
            segments
                .iter()
                .map(|segment| (segment.id.as_str(), segment.text.as_str()))
                .collect::<Vec<_>>(),
            [("dir", "nova"), ("git_branch", "feat…anch")]
        );
    }

    #[test]
    fn fitting_compacts_runtime_segments_to_icons_before_dropping() {
        let mut segments = vec![
            test_segment("dir", "nova"),
            test_segment("rust_version", " 1.96.1"),
        ];

        fit_side(&mut segments, 6, " ");

        assert_eq!(
            segments
                .iter()
                .map(|segment| (segment.id.as_str(), segment.text.as_str()))
                .collect::<Vec<_>>(),
            [("dir", "nova"), ("rust_version", "")]
        );
    }

    #[test]
    fn fitting_drops_runtime_before_compacting_dir() {
        let mut segments = vec![
            test_segment("dir", "~/dev/oss/nova/src/render"),
            test_segment("rust_version", " 1.96.1"),
        ];

        fit_side(&mut segments, 25, " ");

        assert_eq!(
            segments
                .iter()
                .map(|segment| (segment.id.as_str(), segment.text.as_str()))
                .collect::<Vec<_>>(),
            [("dir", "~/dev/oss/nova/src/render")]
        );
    }

    #[test]
    fn fitting_compacts_icon_segments_when_they_fit_with_primary_segments() {
        let mut segments = vec![
            test_segment("dir", "nova"),
            test_segment("nix_shell", " impure (nix-shell-env)"),
            test_segment("aws", " very-long-aws-profile (ap-northeast-1)"),
        ];

        fit_side(&mut segments, 60, " ");

        assert_eq!(
            segments
                .iter()
                .map(|segment| (segment.id.as_str(), segment.text.as_str()))
                .collect::<Vec<_>>(),
            [("dir", "nova"), ("nix_shell", ""), ("aws", "")]
        );
        assert_eq!(lower_side(&segments, " "), "nova  ");
    }

    #[test]
    fn fitting_drops_secondary_segments_before_primary_segments() {
        let mut segments = vec![
            test_segment("rust_version", " 1.96.1"),
            test_segment("nix_shell", " impure (nix-shell-env)"),
            test_segment("aws", " very-long-aws-profile (ap-northeast-1)"),
            test_segment("duration", "+10s"),
            test_segment("prompt_char", "❯ "),
        ];

        fit_side(&mut segments, 8, " ");

        assert_eq!(
            segments
                .iter()
                .map(|segment| (segment.id.as_str(), segment.text.as_str()))
                .collect::<Vec<_>>(),
            [("prompt_char", "❯ ")]
        );
    }

    #[test]
    fn fitting_truncates_the_widest_custom_segment() {
        let mut segments = vec![
            test_segment("custom", "ok"),
            test_segment("custom", "abcdefghijklmnop"),
        ];

        fit_side(&mut segments, 10, " ");

        assert_eq!(side_width(&segments, " "), 10);
        assert_eq!(segments[0].text, "ok");
        assert_eq!(segments[1].text, "abcdef…");
    }

    #[test]
    fn two_line_right_side_never_exceeds_columns() {
        let output = lower(
            RenderedPrompt {
                line1_left: vec![test_segment("custom", "left")],
                line1_right: vec![
                    test_segment("custom", "R"),
                    test_segment(
                        "custom",
                        "right-prompt-that-is-longer-than-the-terminal-width",
                    ),
                ],
                line2_left: vec![test_segment("prompt_char", ">_")],
                line2_right: Vec::new(),
            },
            50,
        );

        let first_line = output.prompt.lines().next().expect("first prompt line");
        assert_eq!(visible_prompt_width(first_line), 50);
    }

    #[test]
    fn fitting_drops_user_host_before_secondary_icons() {
        let mut segments = vec![
            test_segment("user_host", "user@host"),
            test_segment("dir", "directory"),
            test_segment("git_branch", "feature"),
            test_segment("git_status", "[!+?]"),
            test_segment("rust_version", " 1.96.1"),
        ];

        fit_side(&mut segments, 34, " ");

        assert_eq!(
            segments
                .iter()
                .map(|segment| (segment.id.as_str(), segment.text.as_str()))
                .collect::<Vec<_>>(),
            [
                ("dir", "directory"),
                ("git_branch", "feature"),
                ("git_status", "[!+?]"),
                ("rust_version", "")
            ]
        );
    }

    #[test]
    fn fitting_two_line_left_side_drops_secondary_segments_before_truncating_primary() {
        let output = lower(
            RenderedPrompt {
                line1_left: vec![
                    test_segment("dir", "directory"),
                    test_segment("git_branch", "feature/very-long-branch"),
                    test_segment("git_status", "[!+?]"),
                    test_segment("rust_version", " 1.96.1"),
                    test_segment("nix_shell", " impure (nix-shell-env)"),
                ],
                line1_right: vec![test_segment("time", "12:34:56")],
                line2_left: vec![test_segment("prompt_char", ">_")],
                line2_right: Vec::new(),
            },
            50,
        );

        let first_line = output.prompt.lines().next().expect("first prompt line");
        assert!(first_line.contains("directory feature/very-long-branch [!+?]"));
        assert!(!first_line.contains(''));
        assert!(!first_line.contains(''));
        assert!(first_line.ends_with("  12:34:56"));
    }

    #[test]
    fn fitting_strips_git_status_counts_before_dropping_status() {
        let mut segments = vec![
            test_segment("dir", "nova"),
            test_segment("git_status", "[+123?4]"),
        ];

        fit_side(&mut segments, 9, " ");

        assert_eq!(
            segments
                .iter()
                .map(|segment| (segment.id.as_str(), segment.text.as_str()))
                .collect::<Vec<_>>(),
            [("dir", "nova"), ("git_status", "[+?]")]
        );
    }

    #[test]
    fn fitting_uses_compact_dir_path_before_leaf_only_floor() {
        let mut segments = vec![
            test_segment("dir", "~/dev/oss/nova/src/render"),
            test_segment("git_branch", "main"),
        ];

        fit_side(&mut segments, 21, " ");

        assert_eq!(
            segments
                .iter()
                .map(|segment| (segment.id.as_str(), segment.text.as_str()))
                .collect::<Vec<_>>(),
            [("dir", "~/d/o/n/s/render"), ("git_branch", "main")]
        );
    }

    #[test]
    fn fitting_keeps_segments_when_prompt_char_must_wrap_later() {
        let mut segments = vec![
            test_segment("dir", "very-long-directory"),
            test_segment("prompt_char", "❯ "),
        ];

        fit_side(&mut segments, 2, " ");

        assert_eq!(
            segments
                .iter()
                .map(|segment| (segment.id.as_str(), segment.text.as_str()))
                .collect::<Vec<_>>(),
            [("dir", "…y"), ("prompt_char", "❯ ")]
        );
    }

    #[test]
    fn fitting_trims_prompt_char_gap_before_replacing_prompt_char() {
        let mut segments = vec![test_segment("prompt_char", "❯ ")];

        fit_side(&mut segments, 1, " ");

        assert_eq!(
            segments
                .iter()
                .map(|segment| (segment.id.as_str(), segment.text.as_str()))
                .collect::<Vec<_>>(),
            [("prompt_char", "❯")]
        );
    }

    #[test]
    fn fitting_removes_zero_width_segments() {
        let mut segments = vec![
            test_segment("dir", ""),
            test_segment("node_version", "24.16.0"),
        ];

        fit_side(&mut segments, 1, " ");

        assert_no_zero_width_segments(&segments);
    }

    #[test]
    fn fitting_preserves_part_style_when_truncating_segment() {
        let mut segments = vec![SegmentContent::from_parts(
            "user_host",
            vec![
                SegmentPart::new(
                    "user",
                    Style {
                        fg: Some("green".to_string()),
                        bg: None,
                        bold: false,
                    },
                ),
                SegmentPart::new("@host", Style::default()),
            ],
        )];

        fit_side(&mut segments, 4, " ");

        assert_eq!(segments[0].parts[0].style.fg.as_deref(), Some("green"));
        assert_eq!(
            lower_side(&segments, " "),
            "%{\u{1b}[32m%}use…%{\u{1b}[0m%}"
        );
    }

    #[test]
    fn fitting_preserves_part_styles_across_truncation_boundary() {
        let mut segments = vec![SegmentContent::from_parts(
            "user_host",
            vec![
                SegmentPart::new(
                    "user",
                    Style {
                        fg: Some("green".to_string()),
                        bg: None,
                        bold: false,
                    },
                ),
                SegmentPart::new("@host", Style::default()),
            ],
        )];

        fit_side(&mut segments, 6, " ");

        assert_eq!(
            lower_side(&segments, " "),
            "%{\u{1b}[32m%}user%{\u{1b}[0m%}@…"
        );
    }

    #[test]
    fn fitting_preserves_suffix_part_styles_when_truncating_start_side() {
        let segments = vec![SegmentContent::from_parts(
            "user_host",
            vec![
                SegmentPart::new(
                    "abcdef",
                    Style {
                        fg: Some("green".to_string()),
                        bg: None,
                        bold: false,
                    },
                ),
                SegmentPart::new("@host", Style::default()),
            ],
        )];

        let truncated = truncate_start_side(&segments, 8, " ");

        assert_eq!(
            lower_side(&truncated, " "),
            "%{\u{1b}[32m%}…ef%{\u{1b}[0m%}@host"
        );
    }

    #[test]
    fn leading_ellipsis_preserves_single_cell_segment_style() {
        let mut segments = vec![
            SegmentContent::from_parts(
                "custom",
                vec![SegmentPart::new(
                    "x",
                    Style {
                        fg: Some("green".to_string()),
                        bg: None,
                        bold: false,
                    },
                )],
            ),
            test_segment("custom", "tail"),
        ];

        force_leading_ellipsis(&mut segments);

        assert_eq!(segments[0].text, "…");
        assert_eq!(
            lower_side(&segments, " "),
            "%{\u{1b}[32m%}…%{\u{1b}[0m%} tail"
        );
    }

    #[test]
    fn visible_prompt_width_unescapes_literal_percent() {
        assert_eq!(
            strip_prompt_markers("%{\u{1b}[32m%}100%%%{\u{1b}[0m%}"),
            "100%"
        );
        assert_eq!(visible_prompt_width("%{\u{1b}[32m%}100%%%{\u{1b}[0m%}"), 4);
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

    fn test_segment(id: &str, text: &str) -> SegmentContent {
        SegmentContent::new(id, text, Style::default())
    }

    fn assert_no_zero_width_segments(segments: &[SegmentContent]) {
        assert!(
            segments
                .iter()
                .all(|segment| width::display_width(&segment.text) > 0)
        );
    }
}
