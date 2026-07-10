use crate::config::SegmentConfig;
use crate::segments::{SegmentContent, Style, SyncSegment, label_with_icon};
use crate::state::{PromptEnv, PromptState};

const NIX_SHELL_SEGMENT_ID: &str = "nix_shell";
const NIX_SHELL_ICON: &str = "";

pub struct NixShellSegment;

impl SyncSegment for NixShellSegment {
    fn id(&self) -> &'static str {
        NIX_SHELL_SEGMENT_ID
    }

    fn render(&self, state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent> {
        render_nix_shell(&state.env, config)
    }
}

pub fn render_nix_shell(env: &PromptEnv, config: &SegmentConfig) -> Option<SegmentContent> {
    let state = match env.in_nix_shell.as_deref()? {
        "pure" => "pure",
        "impure" => "impure",
        _ => return None,
    };
    let label = match env.nix_shell_name.as_deref() {
        Some(name) => format!("{state} ({name})"),
        None => state.to_string(),
    };

    Some(SegmentContent::new(
        NIX_SHELL_SEGMENT_ID,
        label_with_icon(&label, config, NIX_SHELL_ICON),
        nix_shell_style(config),
    ))
}

fn nix_shell_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        Style::from(&config.style)
    } else {
        Style {
            fg: Some("blue".to_string()),
            bg: None,
            bold: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_nix_shell_segment_for_pure_shell() {
        let segment = render_nix_shell(
            &PromptEnv {
                in_nix_shell: Some("pure".to_string()),
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("nix shell should render");

        assert_eq!(segment.id, "nix_shell");
        assert_eq!(segment.text, " pure");
        assert_eq!(segment.style.fg.as_deref(), Some("blue"));
        assert!(segment.style.bold);
    }

    #[test]
    fn renders_nix_shell_segment_with_name() {
        let segment = render_nix_shell(
            &PromptEnv {
                in_nix_shell: Some("impure".to_string()),
                nix_shell_name: Some("starship".to_string()),
                ..PromptEnv::default()
            },
            &SegmentConfig::default(),
        )
        .expect("nix shell should render");

        assert_eq!(segment.text, " impure (starship)");
    }

    #[test]
    fn omits_nix_shell_segment_for_invalid_state() {
        assert_eq!(
            render_nix_shell(
                &PromptEnv {
                    in_nix_shell: Some("unknown".to_string()),
                    ..PromptEnv::default()
                },
                &SegmentConfig::default(),
            ),
            None
        );
    }
}
