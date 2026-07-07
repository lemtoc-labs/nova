//! Current working directory segment.

use std::path::{Component, Path, PathBuf};

use super::{SegmentContent, Style, SyncSegment};
use crate::config::SegmentConfig;
use crate::state::PromptState;

pub struct DirSegment;

impl SyncSegment for DirSegment {
    fn id(&self) -> &'static str {
        "dir"
    }

    fn render(&self, state: &PromptState, config: &SegmentConfig) -> Option<SegmentContent> {
        let fallback_home = std::env::var_os("HOME").map(PathBuf::from);
        let home = state.env.home.as_deref().or(fallback_home.as_deref());
        let max_components = config.max_components.unwrap_or(4);
        let text = display_path(&state.cwd, home, max_components);
        Some(SegmentContent::new(self.id(), text, dir_style(config)))
    }
}

pub fn display_path(cwd: &Path, home: Option<&Path>, max_components: usize) -> String {
    if let Some(home) = home.filter(|home| cwd.starts_with(home)) {
        let relative = cwd.strip_prefix(home).unwrap_or(cwd);
        return format_with_prefix("~", relative, max_components);
    }

    format_with_prefix(root_prefix(cwd), cwd, max_components)
}

fn format_with_prefix(prefix: &str, path: &Path, max_components: usize) -> String {
    let components = visible_components(path);

    if components.is_empty() {
        return prefix.to_string();
    }

    let shortened = shorten_components(&components, max_components, prefix == "~");
    match prefix {
        "" => shortened.join("/"),
        "/" => format!("/{}", shortened.join("/")),
        "~" => format!("~/{}", shortened.join("/")),
        other => format!("{other}/{}", shortened.join("/")),
    }
}

fn root_prefix(path: &Path) -> &str {
    if path.is_absolute() { "/" } else { "" }
}

fn visible_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            Component::CurDir => Some(".".to_string()),
            Component::ParentDir => Some("..".to_string()),
            Component::Prefix(_) | Component::RootDir => None,
        })
        .collect()
}

fn shorten_components(
    components: &[String],
    max_components: usize,
    abbreviate: bool,
) -> Vec<String> {
    if max_components == 0 {
        return components.to_vec();
    }

    let skipped_count = components.len().saturating_sub(max_components);
    if skipped_count == 0 && !abbreviate {
        return components.to_vec();
    }

    let mut shortened = Vec::new();

    if skipped_count > 0 {
        shortened.push("…".to_string());
    }

    shortened.extend(components.iter().skip(skipped_count).enumerate().map(
        |(index, component)| {
            if index + skipped_count == components.len() - 1 || !abbreviate {
                component.clone()
            } else {
                abbreviate_component(component)
            }
        },
    ));

    shortened
}

fn abbreviate_component(component: &str) -> String {
    if component == "." || component == ".." {
        return component.to_string();
    }

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

fn dir_style(config: &SegmentConfig) -> Style {
    if config.style.fg.is_some() || config.style.bg.is_some() || config.style.bold {
        return Style::from(&config.style);
    }

    Style {
        fg: Some("green".to_string()),
        bg: None,
        bold: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_home_paths() {
        assert_eq!(
            display_path(
                Path::new("/Users/me/dev/oss/nova"),
                Some(Path::new("/Users/me")),
                4
            ),
            "~/d/o/nova"
        );
    }

    #[test]
    fn caps_long_paths_with_ellipsis() {
        assert_eq!(
            display_path(
                Path::new("/Users/me/projects/nova/src/render"),
                Some(Path::new("/Users/me")),
                2
            ),
            "~/…/s/render"
        );
    }

    #[test]
    fn preserves_full_paths_when_component_cap_is_zero() {
        assert_eq!(
            display_path(
                Path::new("/Users/me/projects/nova"),
                Some(Path::new("/Users/me")),
                0
            ),
            "~/projects/nova"
        );
    }

    #[test]
    fn keeps_root_visible() {
        assert_eq!(display_path(Path::new("/"), None, 4), "/");
        assert_eq!(
            display_path(Path::new("/opt/homebrew/bin"), None, 4),
            "/opt/homebrew/bin"
        );
    }
}
