//! Per-shell worker event loop.

pub mod jobs;
pub mod protocol;

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::cache::{CacheKey, SegmentCache};
use crate::config::Config;
use crate::config::load::load_config;
use crate::render::{AsyncSegmentValues, render_with_async};
use crate::segments::SegmentContent;
use crate::segments::git::{collect_git_status, render_git_branch, render_git_status};
use jobs::{JobOutcome, JobPool, JobResult};
use protocol::{
    ClientRecord, FrameDecoder, RenderStatus, WorkerRecord, decode_client_record,
    encode_worker_record,
};

const CACHE_CAPACITY: usize = 128;
const CONFIG_GENERATION: u64 = 0;
const GIT_REFRESH_TTL: Duration = Duration::ZERO;
const GIT_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_CONCURRENCY: usize = 2;

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
    let mut cache = SegmentCache::new(CACHE_CAPACITY);
    let (job_results, job_result_receiver) = mpsc::channel::<JobResult<GitJobSegments>>();
    let job_pool = JobPool::new(MAX_CONCURRENCY, job_results);

    loop {
        let bytes_read = request.read(&mut buffer).map_err(WorkerError::Read)?;
        if bytes_read == 0 {
            return Ok(());
        }

        let chunk = String::from_utf8_lossy(&buffer[..bytes_read]);
        for frame in decoder.push(&chunk) {
            drain_job_results(&job_result_receiver, &mut cache);

            let Ok(ClientRecord::Render(request)) = decode_client_record(&frame) else {
                continue;
            };
            let config = load_config(None).unwrap_or_default();
            let async_values = git_async_values(&cache, &request.state.cwd);
            let output = render_with_async(&config, &request.state, &async_values);
            write_record(
                &mut response,
                &WorkerRecord::Prompt {
                    generation: request.generation,
                    status: RenderStatus::Final,
                    output,
                },
            )?;
            schedule_git_refresh(
                &job_pool,
                &mut cache,
                request.generation,
                request.state.cwd,
                &config,
            );
        }
    }
}

#[derive(Debug)]
struct GitJobSegments {
    branch: Option<SegmentContent>,
    status: Option<SegmentContent>,
}

fn drain_job_results(
    receiver: &mpsc::Receiver<JobResult<GitJobSegments>>,
    cache: &mut SegmentCache,
) {
    while let Ok(result) = receiver.try_recv() {
        let branch_key = git_branch_key_from_status_key(&result.key);
        match result.outcome {
            JobOutcome::Completed(segments) => {
                complete_segment(cache, branch_key, segments.branch, result.finished_at);
                complete_segment(cache, result.key, segments.status, result.finished_at);
            }
            JobOutcome::Panicked => {
                cache.complete_failure(branch_key, result.finished_at);
                cache.complete_failure(result.key, result.finished_at);
            }
        }
    }
}

fn complete_segment(
    cache: &mut SegmentCache,
    key: CacheKey,
    segment: Option<SegmentContent>,
    collected_at: Instant,
) {
    if let Some(segment) = segment {
        cache.complete_success(key, segment, collected_at);
    } else {
        cache.complete_failure(key, collected_at);
    }
}

fn git_async_values(cache: &SegmentCache, cwd: &std::path::Path) -> AsyncSegmentValues {
    let now = Instant::now();
    let status_key = git_status_key(cwd);
    let branch_key = git_branch_key_from_status_key(&status_key);

    AsyncSegmentValues::from([
        (
            "git_branch".to_string(),
            cache.lookup(&branch_key, now, GIT_REFRESH_TTL),
        ),
        (
            "git_status".to_string(),
            cache.lookup(&status_key, now, GIT_REFRESH_TTL),
        ),
    ])
}

fn schedule_git_refresh(
    pool: &JobPool<GitJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    cwd: PathBuf,
    config: &Config,
) {
    let status_key = git_status_key(&cwd);
    if !cache.needs_refresh(&status_key, Instant::now(), GIT_REFRESH_TTL) {
        return;
    }

    cache.mark_inflight(status_key.clone());
    let branch_config = config.segment("git_branch");
    let status_config = config.segment("git_status");
    let spawn_result = pool.spawn(
        generation,
        status_key.clone(),
        GIT_TIMEOUT,
        move |deadline| {
            let Ok(Some(status)) = collect_git_status(&cwd, deadline) else {
                return GitJobSegments {
                    branch: None,
                    status: None,
                };
            };

            GitJobSegments {
                branch: render_git_branch(&status, &branch_config),
                status: render_git_status(&status, &status_config),
            }
        },
    );

    if spawn_result.is_err() {
        cache.complete_failure(status_key, Instant::now());
    }
}

fn git_status_key(cwd: &std::path::Path) -> CacheKey {
    CacheKey::new("git_status", cwd.to_string_lossy(), CONFIG_GENERATION)
}

fn git_branch_key_from_status_key(status_key: &CacheKey) -> CacheKey {
    CacheKey::new(
        "git_branch",
        status_key.source.clone(),
        status_key.config_generation,
    )
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
