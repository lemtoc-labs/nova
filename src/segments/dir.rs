//! Current working directory segment.

use std::env;
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
        let home = env::var_os("HOME").map(PathBuf::from);
        let max_components = config.max_components.unwrap_or(4);
        let text = display_path(&state.cwd, home.as_deref(), max_components);
        Some(SegmentContent::new(
            self.id(),
            text,
            Style::from(&config.style),
        ))
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

    let shortened = shorten_components(&components, max_components);
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

fn shorten_components(components: &[String], max_components: usize) -> Vec<String> {
    if max_components == 0 || components.len() <= max_components {
        return components.to_vec();
    }

    let skipped_count = components.len() - max_components;
    std::iter::once("…".to_string())
        .chain(components.iter().skip(skipped_count).cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_home_paths() {
        assert_eq!(
            display_path(
                Path::new("/Users/me/projects/nova/src/render"),
                Some(Path::new("/Users/me")),
                2
            ),
            "~/…/src/render"
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
