//! Per-shell worker event loop.

pub mod jobs;
pub mod protocol;

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::cache::{CacheKey, SegmentCache};
use crate::config::Config;
use crate::config::load::load_config;
use crate::render::{AsyncSegmentValues, LoweredPrompt, render_with_async};
use crate::segments::SegmentContent;
use crate::segments::git::{collect_git_status, render_git_branch, render_git_status};
use crate::segments::runtime::{collect_rust_version, render_rust_version};
use crate::state::PromptState;
use jobs::{JobOutcome, JobPool, JobResult};
use protocol::{
    ClientRecord, FrameDecoder, RenderStatus, WorkerRecord, decode_client_record,
    encode_worker_record,
};

const CACHE_CAPACITY: usize = 128;
const CONFIG_GENERATION: u64 = 0;
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const GIT_REFRESH_TTL: Duration = Duration::ZERO;
const GIT_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_CONCURRENCY: usize = 2;
const RUNTIME_REFRESH_TTL: Duration = Duration::from_secs(300);
const RUNTIME_TIMEOUT: Duration = Duration::from_secs(1);

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
        .custom_flags(libc::O_NONBLOCK)
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
    let (job_results, job_result_receiver) = mpsc::channel::<JobResult<AsyncJobSegments>>();
    let job_pool = JobPool::new(MAX_CONCURRENCY, job_results);
    let mut active_prompt = None;

    loop {
        drain_job_results(
            &job_result_receiver,
            &mut cache,
            &mut response,
            &mut active_prompt,
        )?;

        match request.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(bytes_read) => {
                let chunk = String::from_utf8_lossy(&buffer[..bytes_read]);
                for frame in decoder.push(&chunk) {
                    let Ok(ClientRecord::Render(request)) = decode_client_record(&frame) else {
                        continue;
                    };
                    let config = load_config(None).unwrap_or_default();
                    let async_values = async_values(&cache, &request.state.cwd, &config);
                    let output = render_with_async(&config, &request.state, &async_values);
                    write_record(
                        &mut response,
                        &WorkerRecord::Prompt {
                            generation: request.generation,
                            status: RenderStatus::Final,
                            output: output.clone(),
                        },
                    )?;

                    active_prompt = Some(ActivePrompt {
                        generation: request.generation,
                        state: request.state.clone(),
                        config: config.clone(),
                        output,
                    });
                    schedule_async_refreshes(
                        &job_pool,
                        &mut cache,
                        request.generation,
                        request.state.cwd,
                        &config,
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                wait_for_job_result(
                    &job_result_receiver,
                    &mut cache,
                    &mut response,
                    &mut active_prompt,
                )?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(WorkerError::Read(error)),
        }
    }
}

#[derive(Clone, Debug)]
struct ActivePrompt {
    generation: u64,
    state: PromptState,
    config: Config,
    output: LoweredPrompt,
}

#[derive(Debug)]
struct AsyncJobSegments {
    segments: Vec<AsyncJobSegment>,
}

#[derive(Debug)]
struct AsyncJobSegment {
    key: CacheKey,
    content: Option<SegmentContent>,
}

fn drain_job_results(
    receiver: &mpsc::Receiver<JobResult<AsyncJobSegments>>,
    cache: &mut SegmentCache,
    response: &mut impl Write,
    active_prompt: &mut Option<ActivePrompt>,
) -> Result<(), WorkerError> {
    while let Ok(result) = receiver.try_recv() {
        handle_job_result(result, cache, response, active_prompt)?;
    }

    Ok(())
}

fn wait_for_job_result(
    receiver: &mpsc::Receiver<JobResult<AsyncJobSegments>>,
    cache: &mut SegmentCache,
    response: &mut impl Write,
    active_prompt: &mut Option<ActivePrompt>,
) -> Result<(), WorkerError> {
    match receiver.recv_timeout(EVENT_POLL_INTERVAL) {
        Ok(result) => handle_job_result(result, cache, response, active_prompt),
        Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => Ok(()),
    }
}

fn handle_job_result(
    result: JobResult<AsyncJobSegments>,
    cache: &mut SegmentCache,
    response: &mut impl Write,
    active_prompt: &mut Option<ActivePrompt>,
) -> Result<(), WorkerError> {
    let generation = result.generation;
    complete_job_result(result, cache);
    write_update_if_active(generation, cache, response, active_prompt)
}

fn complete_job_result(result: JobResult<AsyncJobSegments>, cache: &mut SegmentCache) {
    match result.outcome {
        JobOutcome::Completed(segments) => {
            for segment in segments.segments {
                complete_segment(cache, segment.key, segment.content, result.finished_at);
            }
        }
        JobOutcome::Panicked => {
            if result.key.segment_id == "git_status" {
                cache.complete_failure(
                    git_branch_key_from_status_key(&result.key),
                    result.finished_at,
                );
            }
            cache.complete_failure(result.key, result.finished_at);
        }
    }
}

fn write_update_if_active(
    generation: u64,
    cache: &SegmentCache,
    response: &mut impl Write,
    active_prompt: &mut Option<ActivePrompt>,
) -> Result<(), WorkerError> {
    let Some(active_prompt) = active_prompt else {
        return Ok(());
    };
    if generation != active_prompt.generation {
        return Ok(());
    }

    let async_values = async_values(cache, &active_prompt.state.cwd, &active_prompt.config);
    let output = render_with_async(&active_prompt.config, &active_prompt.state, &async_values);
    if output == active_prompt.output {
        return Ok(());
    }

    write_record(
        response,
        &WorkerRecord::Update {
            generation,
            status: RenderStatus::Final,
            output: output.clone(),
        },
    )?;
    active_prompt.output = output;
    Ok(())
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

fn async_values(
    cache: &SegmentCache,
    cwd: &std::path::Path,
    config: &Config,
) -> AsyncSegmentValues {
    let now = Instant::now();
    let mut values = AsyncSegmentValues::new();

    if config_uses_any_segment(config, &["git_branch", "git_status"]) {
        let status_key = git_status_key(cwd);
        let branch_key = git_branch_key_from_status_key(&status_key);
        values.insert(
            "git_branch".to_string(),
            cache.lookup(&branch_key, now, GIT_REFRESH_TTL),
        );
        values.insert(
            "git_status".to_string(),
            cache.lookup(&status_key, now, GIT_REFRESH_TTL),
        );
    }

    if config_uses_segment(config, "rust_version") {
        let key = rust_version_key(cwd);
        values.insert(
            "rust_version".to_string(),
            cache.lookup(&key, now, RUNTIME_REFRESH_TTL),
        );
    }

    values
}

fn schedule_async_refreshes(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    cwd: PathBuf,
    config: &Config,
) {
    schedule_git_refresh(pool, cache, generation, cwd.clone(), config);
    schedule_rust_refresh(pool, cache, generation, cwd, config);
}

fn schedule_git_refresh(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    cwd: PathBuf,
    config: &Config,
) {
    if !config_uses_any_segment(config, &["git_branch", "git_status"]) {
        return;
    }

    let status_key = git_status_key(&cwd);
    if !cache.needs_refresh(&status_key, Instant::now(), GIT_REFRESH_TTL) {
        return;
    }

    cache.mark_inflight(status_key.clone());
    let branch_key = git_branch_key_from_status_key(&status_key);
    let job_status_key = status_key.clone();
    let job_branch_key = branch_key.clone();
    let branch_config = config.segment("git_branch");
    let status_config = config.segment("git_status");
    let spawn_result = pool.spawn(
        generation,
        status_key.clone(),
        GIT_TIMEOUT,
        move |deadline| {
            let Ok(Some(status)) = collect_git_status(&cwd, deadline) else {
                return AsyncJobSegments {
                    segments: vec![
                        AsyncJobSegment {
                            key: job_branch_key,
                            content: None,
                        },
                        AsyncJobSegment {
                            key: job_status_key,
                            content: None,
                        },
                    ],
                };
            };

            AsyncJobSegments {
                segments: vec![
                    AsyncJobSegment {
                        key: job_branch_key,
                        content: render_git_branch(&status, &branch_config),
                    },
                    AsyncJobSegment {
                        key: job_status_key,
                        content: render_git_status(&status, &status_config),
                    },
                ],
            }
        },
    );

    if spawn_result.is_err() {
        cache.complete_failure(status_key, Instant::now());
    }
}

fn schedule_rust_refresh(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    cwd: PathBuf,
    config: &Config,
) {
    if !config_uses_segment(config, "rust_version") {
        return;
    }

    let key = rust_version_key(&cwd);
    if !cache.needs_refresh(&key, Instant::now(), RUNTIME_REFRESH_TTL) {
        return;
    }

    cache.mark_inflight(key.clone());
    let rust_config = config.segment("rust_version");
    let job_key = key.clone();
    let spawn_result = pool.spawn(generation, key.clone(), RUNTIME_TIMEOUT, move |deadline| {
        let content = collect_rust_version(&cwd, deadline)
            .ok()
            .flatten()
            .and_then(|version| render_rust_version(&version, &rust_config));
        AsyncJobSegments {
            segments: vec![AsyncJobSegment {
                key: job_key,
                content,
            }],
        }
    });

    if spawn_result.is_err() {
        cache.complete_failure(key, Instant::now());
    }
}

fn git_status_key(cwd: &std::path::Path) -> CacheKey {
    CacheKey::new("git_status", cwd.to_string_lossy(), CONFIG_GENERATION)
}

fn rust_version_key(cwd: &std::path::Path) -> CacheKey {
    CacheKey::new("rust_version", cwd.to_string_lossy(), CONFIG_GENERATION)
}

fn git_branch_key_from_status_key(status_key: &CacheKey) -> CacheKey {
    CacheKey::new(
        "git_branch",
        status_key.source.clone(),
        status_key.config_generation,
    )
}

fn config_uses_any_segment(config: &Config, ids: &[&str]) -> bool {
    ids.iter().any(|id| config_uses_segment(config, id))
}

fn config_uses_segment(config: &Config, id: &str) -> bool {
    config.layout.line1.left.iter().any(|segment| segment == id)
        || config
            .layout
            .line1
            .right
            .iter()
            .any(|segment| segment == id)
        || config.layout.line2.left.iter().any(|segment| segment == id)
        || config
            .layout
            .line2
            .right
            .iter()
            .any(|segment| segment == id)
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
