//! Runtime and tool information collectors.

mod aws;
mod bun;
mod deno;
mod detect;
mod nix_shell;
mod node;
mod python;
mod rust;

pub use aws::{AwsSegment, render_aws};
pub use bun::{
    BunSegment, bun_cache_key, collect_bun_version, is_bun_project_dir, parse_bun_version,
    render_bun_version,
};
pub use deno::{
    DenoSegment, collect_deno_version, deno_cache_key, is_deno_project_dir, parse_deno_version,
    render_deno_version,
};
pub use nix_shell::{NixShellSegment, render_nix_shell};
pub use node::{
    NodeSegment, collect_node_version, is_node_project_dir, node_cache_key, parse_node_version,
    render_node_version,
};
pub use python::{
    PythonSegment, collect_python_version, is_python_project_dir, parse_python_version,
    python_cache_key, render_python_version,
};
pub use rust::{
    RustSegment, collect_rust_version, find_rust_project_root, parse_rustc_version,
    render_rust_version, rust_cache_key,
};

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use thiserror::Error;
use wait_timeout::ChildExt;

use crate::cache::CacheKey;
use crate::segments::{AsyncJobSegment, AsyncSegmentFailure, SegmentContent};

const RUNTIME_REFRESH_TTL: Duration = Duration::from_secs(300);
const RUNTIME_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Error)]
pub enum RuntimeCollectError {
    #[error("runtime collection timed out")]
    TimedOut,
    #[error("failed to spawn runtime command: {0}")]
    Spawn(std::io::Error),
    #[error("failed to wait for runtime command: {0}")]
    Wait(std::io::Error),
    #[error("failed to read runtime command output: {0}")]
    ReadOutput(std::io::Error),
    #[error("runtime command output reader panicked")]
    ReaderPanicked,
    #[error("failed to capture runtime command output")]
    MissingStdout,
}

fn collected_version_segment<E>(
    key: CacheKey,
    result: Result<Option<String>, E>,
    render: impl FnOnce(&str) -> Option<SegmentContent>,
) -> Vec<AsyncJobSegment> {
    vec![AsyncJobSegment {
        key,
        content: result
            .map(|version| version.and_then(|version| render(&version)))
            .map_err(|_error| AsyncSegmentFailure::Failed),
    }]
}

#[derive(Clone, Copy)]
struct RuntimeCommandSpec {
    args: &'static [&'static str],
    detect: fn(&Path) -> bool,
    parse: fn(&str) -> Option<String>,
}

fn collect_detected_command_version(
    cwd: &Path,
    deadline: Instant,
    command: &Path,
    path: Option<&str>,
    spec: RuntimeCommandSpec,
) -> Result<Option<String>, RuntimeCollectError> {
    if !(spec.detect)(cwd) {
        return Ok(None);
    }

    let timeout = remaining_time(deadline)?;
    let Some(output) = collect_optional_command_output(command, spec.args, cwd, timeout, path)?
    else {
        return Ok(None);
    };

    Ok((spec.parse)(&String::from_utf8_lossy(&output)))
}

fn collect_command_output(
    command: &Path,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
    path: Option<&str>,
) -> Result<Vec<u8>, RuntimeCollectError> {
    let mut command = runtime_command(command, path);
    let mut child = command
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(RuntimeCollectError::Spawn)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(RuntimeCollectError::MissingStdout)?;
    let stdout_reader = read_stdout(stdout);

    let status = match child
        .wait_timeout(timeout)
        .map_err(RuntimeCollectError::Wait)?
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            return Err(RuntimeCollectError::TimedOut);
        }
    };
    let output = join_stdout(stdout_reader)?;

    if !status.success() {
        return Ok(Vec::new());
    }

    Ok(output)
}

fn collect_optional_command_output(
    command: &Path,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
    path: Option<&str>,
) -> Result<Option<Vec<u8>>, RuntimeCollectError> {
    match collect_command_output(command, args, cwd, timeout, path) {
        Ok(output) => Ok(Some(output)),
        Err(RuntimeCollectError::Spawn(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn runtime_cache_source(source: impl AsRef<str>, path: Option<&str>) -> String {
    let source = source.as_ref();
    match request_path(path) {
        Some(path) => format!("{source}|path={:016x}", path_digest(path)),
        None => source.to_string(),
    }
}

fn path_digest(path: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

fn runtime_command(command: &Path, path: Option<&str>) -> Command {
    let mut command = Command::new(command);
    if let Some(path) = request_path(path) {
        command.env("PATH", path);
    }
    command
}

fn request_path(path: Option<&str>) -> Option<&str> {
    path.filter(|value| !value.is_empty())
}

fn remaining_time(deadline: Instant) -> Result<Duration, RuntimeCollectError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or(RuntimeCollectError::TimedOut)?;

    if remaining.is_zero() {
        Err(RuntimeCollectError::TimedOut)
    } else {
        Ok(remaining)
    }
}

fn read_stdout(mut stdout: std::process::ChildStdout) -> JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output)?;
        Ok(output)
    })
}

fn join_stdout(
    stdout_reader: JoinHandle<std::io::Result<Vec<u8>>>,
) -> Result<Vec<u8>, RuntimeCollectError> {
    stdout_reader
        .join()
        .map_err(|_panic| RuntimeCollectError::ReaderPanicked)?
        .map_err(RuntimeCollectError::ReadOutput)
}

#[cfg(unix)]
#[cfg(test)]
fn write_script(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join(name);
    {
        let mut file = std::fs::File::create(&path).expect("script should be created");
        write!(file, "#!/bin/sh\n{body}").expect("script should be written");
        file.sync_all().expect("script should be synced");
    }
    let mut permissions = std::fs::metadata(&path)
        .expect("script metadata should be read")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("script should be executable");
    let file = std::fs::File::open(&path).expect("script should be opened for sync");
    file.sync_all()
        .expect("script should be synced after chmod");
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_command_uses_request_path_when_present() {
        let command = runtime_command(Path::new("node"), Some("/request/bin:/usr/bin"));
        let path = command
            .get_envs()
            .find_map(|(key, value)| (key == "PATH").then_some(value))
            .flatten();

        assert_eq!(path, Some(std::ffi::OsStr::new("/request/bin:/usr/bin")));
    }

    #[test]
    fn runtime_command_leaves_path_unset_when_missing() {
        let command = runtime_command(Path::new("node"), None);

        assert!(command.get_envs().all(|(key, _value)| key != "PATH"));
    }
}
