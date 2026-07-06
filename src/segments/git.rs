//! Git branch and status collectors.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitStatus {
    pub branch: Option<String>,
    pub head_oid: Option<String>,
    pub staged: usize,
    pub modified: usize,
    pub untracked: usize,
    pub conflicted: usize,
    pub stashed: usize,
    pub ahead: usize,
    pub behind: usize,
}

impl GitStatus {
    pub fn has_changes(&self) -> bool {
        self.staged > 0
            || self.modified > 0
            || self.untracked > 0
            || self.conflicted > 0
            || self.stashed > 0
            || self.ahead > 0
            || self.behind > 0
    }
}

pub fn parse_porcelain_v2_z(output: &[u8]) -> GitStatus {
    let mut status = GitStatus::default();

    for record in output.split(|byte| *byte == b'\0') {
        if record.is_empty() {
            continue;
        }

        let record = String::from_utf8_lossy(record);
        parse_record(&record, &mut status);
    }

    status
}

fn parse_record(record: &str, status: &mut GitStatus) {
    if let Some(oid) = record.strip_prefix("# branch.oid ") {
        status.head_oid = non_empty_value(oid);
    } else if let Some(branch) = record.strip_prefix("# branch.head ") {
        status.branch = parse_branch(branch);
    } else if let Some(counts) = record.strip_prefix("# branch.ab ") {
        parse_ahead_behind(counts, status);
    } else if let Some(stash_count) = record.strip_prefix("# stash ") {
        status.stashed = stash_count.trim().parse().unwrap_or_default();
    } else if record.starts_with("1 ") || record.starts_with("2 ") {
        parse_changed_entry(record, status);
    } else if record.starts_with("u ") {
        status.conflicted += 1;
    } else if record.starts_with("? ") {
        status.untracked += 1;
    }
}

fn non_empty_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_branch(value: &str) -> Option<String> {
    match value.trim() {
        "" | "(detached)" => None,
        branch => Some(branch.to_string()),
    }
}

fn parse_ahead_behind(counts: &str, status: &mut GitStatus) {
    for count in counts.split_whitespace() {
        if let Some(ahead) = count.strip_prefix('+') {
            status.ahead = ahead.parse().unwrap_or_default();
        } else if let Some(behind) = count.strip_prefix('-') {
            status.behind = behind.parse().unwrap_or_default();
        }
    }
}

fn parse_changed_entry(record: &str, status: &mut GitStatus) {
    let Some(xy) = record.split_whitespace().nth(1) else {
        return;
    };

    let mut chars = xy.chars();
    let index_status = chars.next();
    let worktree_status = chars.next();

    if index_status.is_some_and(|status| status != '.') {
        status.staged += 1;
    }
    if worktree_status.is_some_and(|status| status != '.') {
        status.modified += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::*;

    #[test]
    fn parses_branch_and_counts_from_porcelain_v2_z() {
        let output = b"# branch.oid abc123def456\0\
# branch.head main\0\
# branch.ab +1 -2\0\
1 M. N... 000000 000000 000000 abc123 def456 staged.rs\0\
1 .M N... 000000 000000 000000 abc123 def456 modified.rs\0\
? untracked.txt\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.branch.as_deref(), Some("main"));
        assert_eq!(status.head_oid.as_deref(), Some("abc123def456"));
        assert_eq!(status.ahead, 1);
        assert_eq!(status.behind, 2);
        assert_eq!(status.staged, 1);
        assert_eq!(status.modified, 1);
        assert_eq!(status.untracked, 1);
        assert_eq!(status.conflicted, 0);
    }

    #[test]
    fn parses_detached_head_without_branch() {
        let output = b"# branch.oid abc123\0# branch.head (detached)\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.branch, None);
        assert_eq!(status.head_oid.as_deref(), Some("abc123"));
    }

    #[test]
    fn parses_staged_and_modified_from_single_entry() {
        let output = b"# branch.head feature\0\
1 MM N... 000000 000000 000000 abc123 def456 both.rs\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.staged, 1);
        assert_eq!(status.modified, 1);
    }

    #[test]
    fn parses_rename_as_staged_change() {
        let output = b"# branch.head main\0\
2 R. N... 000000 000000 000000 abc123 def456 R100 new.rs\0old.rs\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.staged, 1);
        assert_eq!(status.modified, 0);
    }

    #[test]
    fn parses_conflicted_entries() {
        let output = b"# branch.head main\0\
u UU N... 000000 000000 000000 100644 100644 100644 abc123 def456 ghi789 conflict.rs\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.conflicted, 1);
    }

    #[test]
    fn parses_stash_count() {
        let output = b"# branch.head main\0# stash 3\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.stashed, 3);
    }

    #[test]
    fn malformed_counts_fall_back_to_zero() {
        let output = b"# branch.head main\0# branch.ab +x -y\0# stash nope\0";

        let status = parse_porcelain_v2_z(output);

        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
        assert_eq!(status.stashed, 0);
        assert!(!status.has_changes());
    }

    #[test]
    fn empty_output_returns_default_status() {
        assert_eq!(parse_porcelain_v2_z(b""), GitStatus::default());
    }

    #[test]
    fn parses_real_git_status_output_with_staged_file() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        init_repo(dir.path())?;
        std::fs::write(dir.path().join("staged.txt"), "hello")?;
        run_git(dir.path(), &["add", "staged.txt"])?;

        let output = run_git(
            dir.path(),
            &[
                "--no-optional-locks",
                "status",
                "--porcelain=v2",
                "--branch",
                "--show-stash",
                "-z",
            ],
        )?;
        let status = parse_porcelain_v2_z(&output);

        assert!(status.branch.is_some(), "branch should be reported");
        assert_eq!(status.staged, 1);
        assert!(status.has_changes());
        Ok(())
    }

    fn init_repo(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let init = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(path)
            .output()?;

        if init.status.success() {
            return Ok(());
        }

        run_git(path, &["init"])?;
        Ok(())
    }

    fn run_git(path: &Path, args: &[&str]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let output = Command::new("git").args(args).current_dir(path).output()?;
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(output.stdout)
    }
}
