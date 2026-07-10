//! Display width and truncation helpers.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn display_width(input: &str) -> usize {
    UnicodeWidthStr::width(input)
}

pub fn truncate_end(input: &str, max_width: usize) -> String {
    truncate(input, max_width, TruncateSide::End)
}

pub fn truncate_start(input: &str, max_width: usize) -> String {
    truncate(input, max_width, TruncateSide::Start)
}

pub fn truncate_middle(input: &str, max_width: usize) -> String {
    if display_width(input) <= max_width {
        return input.to_string();
    }

    if max_width == 0 {
        return String::new();
    }

    if max_width == 1 {
        return "…".to_string();
    }

    let content_width = max_width - 1;
    let prefix_width = content_width / 2;
    let prefix = take_prefix(input, prefix_width);
    let suffix_width = content_width.saturating_sub(display_width(&prefix));
    let suffix = take_suffix(input, suffix_width);
    format!("{prefix}…{suffix}")
}

fn truncate(input: &str, max_width: usize, side: TruncateSide) -> String {
    if display_width(input) <= max_width {
        return input.to_string();
    }

    if max_width == 0 {
        return String::new();
    }

    if max_width == 1 {
        return "…".to_string();
    }

    let content_width = max_width - 1;
    match side {
        TruncateSide::End => {
            let prefix = take_prefix(input, content_width);
            format!("{prefix}…")
        }
        TruncateSide::Start => {
            let suffix = take_suffix(input, content_width);
            format!("…{suffix}")
        }
    }
}

pub(super) fn take_prefix(input: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut output = String::new();

    for character in input.chars() {
        let char_width = character.width().unwrap_or(0);
        if width + char_width > max_width {
            break;
        }
        output.push(character);
        width += char_width;
    }

    output
}

pub(super) fn take_suffix(input: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut output = Vec::new();

    for character in input.chars().rev() {
        let char_width = character.width().unwrap_or(0);
        if width + char_width > max_width {
            break;
        }
        output.push(character);
        width += char_width;
    }

    output.into_iter().rev().collect()
}

#[derive(Clone, Copy, Debug)]
enum TruncateSide {
    Start,
    End,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_without_exceeding_width() {
        assert_eq!(truncate_end("abcdef", 4), "abc…");
        assert_eq!(truncate_start("abcdef", 4), "…def");
        assert_eq!(truncate_middle("abcdef", 4), "a…ef");
        assert!(display_width(&truncate_end("日本語abcdef", 6)) <= 6);
    }
}
