//! zsh prompt lowering and escaping.

use crate::segments::{SegmentContent, Style};

pub fn lower_segment(segment: &SegmentContent) -> String {
    let text = escape_prompt_text(&segment.text);
    let Some(start) = start_sgr(&segment.style) else {
        return text;
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
}
