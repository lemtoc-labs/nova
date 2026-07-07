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
        codes.push("1".to_string());
    }

    if let Some(code) = style
        .fg
        .as_deref()
        .and_then(|color| color_sgr(color, false))
    {
        codes.push(code);
    }

    if let Some(code) = style.bg.as_deref().and_then(|color| color_sgr(color, true)) {
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

fn color_sgr(color: &str, bg: bool) -> Option<String> {
    if let Some(code) = named_color_sgr(color, bg) {
        return Some(code.to_string());
    }

    if let Ok(index) = color.parse::<u8>() {
        let prefix = if bg { "48" } else { "38" };
        return Some(format!("{prefix};5;{index}"));
    }

    let (red, green, blue) = parse_truecolor(color)?;
    let prefix = if bg { "48" } else { "38" };
    Some(format!("{prefix};2;{red};{green};{blue}"))
}

fn named_color_sgr(color: &str, bg: bool) -> Option<&'static str> {
    match (color, bg) {
        ("black", false) => Some("30"),
        ("red", false) => Some("31"),
        ("green", false) => Some("32"),
        ("yellow", false) => Some("33"),
        ("blue", false) => Some("34"),
        ("magenta", false) => Some("35"),
        ("cyan", false) => Some("36"),
        ("white", false) => Some("37"),
        ("bright_black", false) => Some("90"),
        ("bright_red", false) => Some("91"),
        ("bright_green", false) => Some("92"),
        ("bright_yellow", false) => Some("93"),
        ("bright_blue", false) => Some("94"),
        ("bright_magenta", false) => Some("95"),
        ("bright_cyan", false) => Some("96"),
        ("bright_white", false) => Some("97"),
        ("black", true) => Some("40"),
        ("red", true) => Some("41"),
        ("green", true) => Some("42"),
        ("yellow", true) => Some("43"),
        ("blue", true) => Some("44"),
        ("magenta", true) => Some("45"),
        ("cyan", true) => Some("46"),
        ("white", true) => Some("47"),
        ("bright_black", true) => Some("100"),
        ("bright_red", true) => Some("101"),
        ("bright_green", true) => Some("102"),
        ("bright_yellow", true) => Some("103"),
        ("bright_blue", true) => Some("104"),
        ("bright_magenta", true) => Some("105"),
        ("bright_cyan", true) => Some("106"),
        ("bright_white", true) => Some("107"),
        _ => None,
    }
}

fn parse_truecolor(color: &str) -> Option<(u8, u8, u8)> {
    if color.len() != 7
        || !color.starts_with('#')
        || !color[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }

    let red = u8::from_str_radix(&color[1..3], 16).ok()?;
    let green = u8::from_str_radix(&color[3..5], 16).ok()?;
    let blue = u8::from_str_radix(&color[5..7], 16).ok()?;
    Some((red, green, blue))
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

    #[test]
    fn lowers_256_color_styles() {
        let segment = SegmentContent::new(
            "test",
            "indexed",
            Style {
                fg: Some("202".to_string()),
                bg: Some("17".to_string()),
                bold: false,
            },
        );

        assert_eq!(
            lower_segment(&segment),
            "%{\u{1b}[38;5;202;48;5;17m%}indexed%{\u{1b}[0m%}"
        );
    }

    #[test]
    fn lowers_truecolor_styles() {
        let segment = SegmentContent::new(
            "test",
            "rgb",
            Style {
                fg: Some("#ff8800".to_string()),
                bg: Some("#102030".to_string()),
                bold: true,
            },
        );

        assert_eq!(
            lower_segment(&segment),
            "%{\u{1b}[1;38;2;255;136;0;48;2;16;32;48m%}rgb%{\u{1b}[0m%}"
        );
    }

    #[test]
    fn ignores_invalid_truecolor_styles() {
        let invalid_color =
            String::from_utf8(vec![b'#', b'1', 0xc3, 0xa9, b'2', b'3', b'4']).unwrap();
        let segment = SegmentContent::new(
            "test",
            "invalid",
            Style {
                fg: Some(invalid_color),
                bg: Some("#gggggg".to_string()),
                bold: false,
            },
        );

        assert_eq!(lower_segment(&segment), "invalid");
    }
}
