//! Per-shell worker event loop.

pub mod jobs;
pub mod protocol;

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;

use thiserror::Error;

use crate::config::load::load_config;
use crate::render::render;
use protocol::{
    ClientRecord, FrameDecoder, RenderStatus, WorkerRecord, decode_client_record,
    encode_worker_record,
};

#[derive(Clone, Debug)]
pub struct WorkerOptions {
    pub runtime_dir: PathBuf,
    pub session_token: String,
}

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("failed to open request FIFO `{path}`: {source}")]
    OpenRequest {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to open response FIFO `{path}`: {source}")]
    OpenResponse {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write worker response: {0}")]
    Write(std::io::Error),
    #[error("failed to read worker request: {0}")]
    Read(std::io::Error),
}

pub fn run(options: WorkerOptions) -> Result<(), WorkerError> {
    let request_path = options.runtime_dir.join("req");
    let response_path = options.runtime_dir.join("resp");
    let mut request = OpenOptions::new()
        .read(true)
        .open(&request_path)
        .map_err(|source| WorkerError::OpenRequest {
            path: request_path,
            source,
        })?;
    let mut response = OpenOptions::new()
        .write(true)
        .open(&response_path)
        .map_err(|source| WorkerError::OpenResponse {
            path: response_path,
            source,
        })?;

    write_record(
        &mut response,
        &WorkerRecord::Handshake {
            session_token: options.session_token,
        },
    )?;

    let mut decoder = FrameDecoder::default();
    let mut buffer = [0_u8; 4096];

    loop {
        let bytes_read = request.read(&mut buffer).map_err(WorkerError::Read)?;
        if bytes_read == 0 {
            return Ok(());
        }

        let chunk = String::from_utf8_lossy(&buffer[..bytes_read]);
        for frame in decoder.push(&chunk) {
            let Ok(ClientRecord::Render(request)) = decode_client_record(&frame) else {
                continue;
            };
            let config = load_config(None).unwrap_or_default();
            let output = render(&config, &request.state);
            write_record(
                &mut response,
                &WorkerRecord::Prompt {
                    generation: request.generation,
                    status: RenderStatus::Final,
                    output,
                },
            )?;
        }
    }
}

fn write_record<W>(writer: &mut W, record: &WorkerRecord) -> Result<(), WorkerError>
where
    W: Write,
{
    writer
        .write_all(encode_worker_record(record).as_bytes())
        .map_err(WorkerError::Write)?;
    writer.flush().map_err(WorkerError::Write)
}
