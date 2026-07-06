#![cfg(unix)]

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
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
    let mut response = open_fifo_read(runtime_dir.join("resp"));

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
fn worker_renders_cached_git_status_on_later_prompts() {
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
    let mut response = open_fifo_read(runtime_dir.join("resp"));

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

    for generation in 2..20 {
        thread::sleep(Duration::from_millis(20));
        write_render_request(&mut request, generation, repo.path().to_path_buf(), 160);
        let output = read_prompt_output(&mut response, generation);
        if output.prompt.contains("main") && output.prompt.contains("[+1]") {
            drop(request);
            drop(response);
            assert_worker_exits(&mut child);
            return;
        }
    }

    drop(request);
    drop(response);
    let _ = child.kill();
    let _ = child.wait();
    panic!("cached git status did not appear on later prompts");
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
    response: &mut fs::File,
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

fn read_worker_record(response: &mut fs::File) -> WorkerRecord {
    let mut decoder = FrameDecoder::default();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut buffer = [0_u8; 1024];

    loop {
        match response.read(&mut buffer) {
            Ok(0) => {}
            Ok(bytes_read) => {
                let chunk = String::from_utf8_lossy(&buffer[..bytes_read]);
                if let Some(frame) = decoder.push(&chunk).into_iter().next() {
                    return decode_worker_record(&frame).expect("worker record should decode");
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
