use std::fs;
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) struct RuntimeDetection<'a> {
    pub(super) files: &'a [&'a str],
    pub(super) excluded_files: &'a [&'a str],
    pub(super) folders: &'a [&'a str],
    pub(super) excluded_folders: &'a [&'a str],
    pub(super) extensions: &'a [&'a str],
}

pub(super) fn current_dir_matches(cwd: &Path, detection: RuntimeDetection<'_>) -> bool {
    let Ok(entries) = fs::read_dir(cwd) else {
        return false;
    };

    let mut has_positive_match = false;
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let is_dir = entry.file_type().is_ok_and(|file_type| file_type.is_dir());

        if is_dir {
            if detection.excluded_folders.contains(&file_name) {
                return false;
            }
            if detection.folders.contains(&file_name) {
                has_positive_match = true;
            }
        } else {
            if detection.excluded_files.contains(&file_name) {
                return false;
            }
            if detection.files.contains(&file_name)
                || file_has_any_extension(file_name, detection.extensions)
            {
                has_positive_match = true;
            }
        }
    }

    has_positive_match
}

fn file_has_any_extension(file_name: &str, extensions: &[&str]) -> bool {
    if extensions.is_empty() || file_name.starts_with('.') {
        return false;
    }

    let path = Path::new(file_name);
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extensions.contains(&extension))
        || file_name
            .split_once('.')
            .is_some_and(|(_name, extension)| extensions.contains(&extension))
}
