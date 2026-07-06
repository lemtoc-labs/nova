#![cfg(unix)]

use std::collections::VecDeque;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::cargo::cargo_bin;
use nova::state::{Keymap, PromptState};
use nova::worker::protocol::{
    ClientRecord, FrameDecoder, RenderRequest, RenderStatus, WorkerRecord, decode_worker_record,
    encode_client_record,
};
use wait_timeout::ChildExt;

#[test]
fn worker_renders_prompt_over_fifos() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let runtime_dir = tempdir.path();
    create_fifo(runtime_dir.join("req"));
    create_fifo(runtime_dir.join("resp"));

    let mut child = StdCommand::new(cargo_bin("nova"))
        .arg("worker")
        .arg("--dir")
        .arg(runtime_dir)
        .arg("--session-token")
        .arg("test-token")
        .spawn()
        .expect("worker should spawn");

    let mut request = open_fifo_write(runtime_dir.join("req"));
    let mut response = WorkerReader::new(open_fifo_read(runtime_dir.join("resp")));

    assert_eq!(
        read_worker_record(&mut response),
        WorkerRecord::Handshake {
            session_token: "test-token".to_string()
        }
    );

    let request_record = ClientRecord::Render(RenderRequest {
        generation: 3,
        state: PromptState {
            cwd: PathBuf::from("/tmp/nova"),
            exit_status: 1,
            duration_ms: Some(2_500),
            columns: 80,
            keymap: Keymap::Main,
        },
    });
    request
        .write_all(encode_client_record(&request_record).as_bytes())
        .expect("request should be written");

    let response_record = read_worker_record(&mut response);
    let WorkerRecord::Prompt {
        generation,
        status,
        output,
    } = response_record
    else {
        panic!("expected prompt response");
    };

    assert_eq!(generation, 3);
    assert_eq!(status, RenderStatus::Final);
    assert!(output.prompt.contains("/tmp/nova"));
    assert!(output.prompt.contains("❯"));

    drop(request);
    drop(response);
    assert_worker_exits(&mut child);
}

#[test]
fn worker_sends_update_when_git_status_finishes() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let runtime_dir = tempdir.path().join("runtime");
    fs::create_dir(&runtime_dir).expect("runtime dir should be created");
    create_fifo(runtime_dir.join("req"));
    create_fifo(runtime_dir.join("resp"));

    let repo = tempfile::tempdir().expect("repo tempdir should be created");
    init_git_repo(repo.path());
    fs::write(repo.path().join("staged.txt"), "hello").expect("file should be written");
    run_git(repo.path(), &["add", "staged.txt"]);

    let mut child = StdCommand::new(cargo_bin("nova"))
        .arg("worker")
        .arg("--dir")
        .arg(&runtime_dir)
        .arg("--session-token")
        .arg("test-token")
        .spawn()
        .expect("worker should spawn");

    let mut request = open_fifo_write(runtime_dir.join("req"));
    let mut response = WorkerReader::new(open_fifo_read(runtime_dir.join("resp")));

    assert_eq!(
        read_worker_record(&mut response),
        WorkerRecord::Handshake {
            session_token: "test-token".to_string()
        }
    );

    write_render_request(&mut request, 1, repo.path().to_path_buf(), 160);
    let first_output = read_prompt_output(&mut response, 1);
    assert!(
        !first_output.prompt.contains("[+1]"),
        "first render should not block on git status: {}",
        first_output.prompt
    );

    let update_output = read_update_output(&mut response, 1);
    assert!(update_output.prompt.contains("main"));
    assert!(update_output.prompt.contains("[+1]"));

    drop(request);
    drop(response);
    assert_worker_exits(&mut child);
}

#[test]
fn worker_sends_update_when_rust_version_finishes() {
    let tempdir = tempfile::tempdir().expect("tempdir should be created");
    let runtime_dir = tempdir.path().join("runtime");
    fs::create_dir(&runtime_dir).expect("runtime dir should be created");
    create_fifo(runtime_dir.join("req"));
    create_fifo(runtime_dir.join("resp"));

    let project = tempdir.path().join("project");
    fs::create_dir(&project).expect("project dir should be created");
    fs::write(project.join("Cargo.toml"), "[package]\nname = \"demo\"\n")
        .expect("Cargo.toml should be written");

    let config_path = tempdir.path().join("nova.toml");
    fs::write(
        &config_path,
        r#"
        [layout]
        lines = 2

        [layout.line1]
        left = ["dir", "rust_version"]
        right = []

        [layout.line2]
        left = ["prompt_char"]
        right = []
        "#,
    )
    .expect("config should be written");

    let bin_dir = tempdir.path().join("bin");
    fs::create_dir(&bin_dir).expect("bin dir should be created");
    write_script(&bin_dir, "rustc", "printf 'rustc 1.96.1 (abc date)\\n'\n");

    let path = format!(
        "{}:{}",
        bin_dir.to_string_lossy(),
        env::var("PATH").unwrap_or_default()
    );
    let mut child = StdCommand::new(cargo_bin("nova"))
        .arg("worker")
        .arg("--dir")
        .arg(&runtime_dir)
        .arg("--session-token")
        .arg("test-token")
        .env("NOVA_CONFIG", &config_path)
        .env("PATH", path)
        .spawn()
        .expect("worker should spawn");

    let mut request = open_fifo_write(runtime_dir.join("req"));
    let mut response = WorkerReader::new(open_fifo_read(runtime_dir.join("resp")));

    assert_eq!(
        read_worker_record(&mut response),
        WorkerRecord::Handshake {
            session_token: "test-token".to_string()
        }
    );

    write_render_request(&mut request, 1, project, 160);
    let first_output = read_prompt_output(&mut response, 1);
    assert!(
        !first_output.prompt.contains("rust 1.96.1"),
        "first render should not block on rust version: {}",
        first_output.prompt
    );

    let update_output = read_update_output(&mut response, 1);
    assert!(update_output.prompt.contains("rust 1.96.1"));

    drop(request);
    drop(response);
    assert_worker_exits(&mut child);
}

fn create_fifo(path: impl AsRef<Path>) {
    let status = StdCommand::new("mkfifo")
        .arg(path.as_ref())
        .status()
        .expect("mkfifo should run");
    assert!(status.success(), "mkfifo should succeed");
}

fn write_render_request(request: &mut fs::File, generation: u64, cwd: PathBuf, columns: u16) {
    let request_record = ClientRecord::Render(RenderRequest {
        generation,
        state: PromptState {
            cwd,
            exit_status: 0,
            duration_ms: None,
            columns,
            keymap: Keymap::Main,
        },
    });
    request
        .write_all(encode_client_record(&request_record).as_bytes())
        .expect("request should be written");
}

fn read_prompt_output(
    response: &mut WorkerReader,
    expected_generation: u64,
) -> nova::render::LoweredPrompt {
    let WorkerRecord::Prompt {
        generation,
        status,
        output,
    } = read_worker_record(response)
    else {
        panic!("expected prompt response");
    };

    assert_eq!(generation, expected_generation);
    assert_eq!(status, RenderStatus::Final);
    output
}

fn read_update_output(
    response: &mut WorkerReader,
    expected_generation: u64,
) -> nova::render::LoweredPrompt {
    let WorkerRecord::Update {
        generation,
        status,
        output,
    } = read_worker_record(response)
    else {
        panic!("expected update response");
    };

    assert_eq!(generation, expected_generation);
    assert_eq!(status, RenderStatus::Final);
    output
}

fn open_fifo_write(path: impl AsRef<Path>) -> fs::File {
    retry_until_timeout(|| {
        OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path.as_ref())
    })
}

fn open_fifo_read(path: impl AsRef<Path>) -> fs::File {
    retry_until_timeout(|| {
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path.as_ref())
    })
}

struct WorkerReader {
    response: fs::File,
    decoder: FrameDecoder,
    pending: VecDeque<WorkerRecord>,
}

impl WorkerReader {
    fn new(response: fs::File) -> Self {
        Self {
            response,
            decoder: FrameDecoder::default(),
            pending: VecDeque::new(),
        }
    }
}

fn read_worker_record(reader: &mut WorkerReader) -> WorkerRecord {
    if let Some(record) = reader.pending.pop_front() {
        return record;
    }

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut buffer = [0_u8; 1024];

    loop {
        match reader.response.read(&mut buffer) {
            Ok(0) => {}
            Ok(bytes_read) => {
                let chunk = String::from_utf8_lossy(&buffer[..bytes_read]);
                for frame in reader.decoder.push(&chunk) {
                    reader.pending.push_back(
                        decode_worker_record(&frame).expect("worker record should decode"),
                    );
                }
                if let Some(record) = reader.pending.pop_front() {
                    return record;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("failed to read worker response: {error}"),
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for worker record"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn retry_until_timeout<T, F>(mut operation: F) -> T
where
    F: FnMut() -> std::io::Result<T>,
{
    let deadline = Instant::now() + Duration::from_secs(2);

    loop {
        match operation() {
            Ok(value) => return value,
            Err(error) if is_retryable_open_error(&error) => {}
            Err(error) => panic!("operation failed: {error}"),
        }

        assert!(Instant::now() < deadline, "timed out waiting for FIFO");
        thread::sleep(Duration::from_millis(10));
    }
}

fn is_retryable_open_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::NotFound
    ) || error.raw_os_error() == Some(libc::ENXIO)
}

fn init_git_repo(path: &Path) {
    let init = StdCommand::new("git")
        .args(["init", "-b", "main"])
        .current_dir(path)
        .output()
        .expect("git init should run");

    if init.status.success() {
        return;
    }

    run_git(path, &["init"]);
    run_git(path, &["checkout", "-b", "main"]);
}

fn run_git(path: &Path, args: &[&str]) {
    let output = StdCommand::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .expect("git should run");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}")).expect("script should be written");
    let mut permissions = fs::metadata(&path)
        .expect("script metadata should be read")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("script should be executable");
    path
}

fn assert_worker_exits(child: &mut std::process::Child) {
    let status = child
        .wait_timeout(Duration::from_secs(2))
        .expect("worker wait should succeed");

    if let Some(status) = status {
        assert!(status.success(), "worker should exit cleanly");
    } else {
        child.kill().expect("worker should be killed");
        panic!("worker did not exit after request FIFO closed");
    }
}
