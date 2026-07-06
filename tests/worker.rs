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

fn create_fifo(path: impl AsRef<Path>) {
    let status = StdCommand::new("mkfifo")
        .arg(path.as_ref())
        .status()
        .expect("mkfifo should run");
    assert!(status.success(), "mkfifo should succeed");
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
