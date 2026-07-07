//! zsh prompt lowering and escaping.

use crate::segments::{SegmentContent, SegmentPart, Style};

pub fn lower_segment(segment: &SegmentContent) -> String {
    if segment.uses_parts() {
        return segment
            .parts
            .iter()
            .map(lower_part)
            .collect::<Vec<_>>()
            .join("");
    }

    let text = escape_prompt_text(&segment.text);
    lower_text(&text, &segment.style)
}

fn lower_part(part: &SegmentPart) -> String {
    let text = escape_prompt_text(&part.text);
    lower_text(&text, &part.style)
}

fn lower_text(text: &str, style: &Style) -> String {
    let Some(start) = start_sgr(style) else {
        return text.to_string();
    };

    format!(
        "{}{}{}",
        wrap_non_printing(&start),
        text,
        wrap_non_printing("\x1b[0m")
    )
}

pub fn escape_prompt_text(input: &str) -> String {
    input.replace('%', "%%")
}

fn start_sgr(style: &Style) -> Option<String> {
    let mut codes = Vec::new();

    if style.bold {
        codes.push("1");
    }

    if let Some(code) = style.fg.as_deref().and_then(fg_code) {
        codes.push(code);
    }

    if let Some(code) = style.bg.as_deref().and_then(bg_code) {
        codes.push(code);
    }

    if codes.is_empty() {
        None
    } else {
        Some(format!("\x1b[{}m", codes.join(";")))
    }
}

fn wrap_non_printing(input: &str) -> String {
    format!("%{{{input}%}}")
}

fn fg_code(color: &str) -> Option<&'static str> {
    match color {
        "black" => Some("30"),
        "red" => Some("31"),
        "green" => Some("32"),
        "yellow" => Some("33"),
        "blue" => Some("34"),
        "magenta" => Some("35"),
        "cyan" => Some("36"),
        "white" => Some("37"),
        "bright_black" => Some("90"),
        "bright_red" => Some("91"),
        "bright_green" => Some("92"),
        "bright_yellow" => Some("93"),
        "bright_blue" => Some("94"),
        "bright_magenta" => Some("95"),
        "bright_cyan" => Some("96"),
        "bright_white" => Some("97"),
        _ => None,
    }
}

fn bg_code(color: &str) -> Option<&'static str> {
    match color {
        "black" => Some("40"),
        "red" => Some("41"),
        "green" => Some("42"),
        "yellow" => Some("43"),
        "blue" => Some("44"),
        "magenta" => Some("45"),
        "cyan" => Some("46"),
        "white" => Some("47"),
        "bright_black" => Some("100"),
        "bright_red" => Some("101"),
        "bright_green" => Some("102"),
        "bright_yellow" => Some("103"),
        "bright_blue" => Some("104"),
        "bright_magenta" => Some("105"),
        "bright_cyan" => Some("106"),
        "bright_white" => Some("107"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_percent_and_wraps_ansi_sequences() {
        let segment = SegmentContent::new(
            "test",
            "100%",
            Style {
                fg: Some("red".to_string()),
                bg: None,
                bold: true,
            },
        );

        assert_eq!(
            lower_segment(&segment),
            "%{\u{1b}[1;31m%}100%%%{\u{1b}[0m%}"
        );
    }

    #[test]
    fn lowers_styled_parts_without_inserting_spaces() {
        let segment = SegmentContent::from_parts(
            "user_host",
            vec![
                SegmentPart::new(
                    "nova",
                    Style {
                        fg: Some("green".to_string()),
                        bg: None,
                        bold: false,
                    },
                ),
                SegmentPart::new("@M4Pro", Style::default()),
            ],
        );

        assert_eq!(
            lower_segment(&segment),
            "%{\u{1b}[32m%}nova%{\u{1b}[0m%}@M4Pro"
        );
    }

    #[test]
    fn lowers_bright_ansi_colors() {
        let segment = SegmentContent::new(
            "test",
            "bright",
            Style {
                fg: Some("bright_white".to_string()),
                bg: Some("bright_blue".to_string()),
                bold: false,
            },
        );

        assert_eq!(
            lower_segment(&segment),
            "%{\u{1b}[97;104m%}bright%{\u{1b}[0m%}"
        );
    }
}
