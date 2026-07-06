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

use crate::cache::{AsyncValue, CacheKey, SegmentCache};
use crate::config::load::load_config;
use crate::config::{Config, SegmentConfig};
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
    let mut config_state = ConfigState::default();
    let mut warned_config_error = false;

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
                    let worker_config =
                        load_worker_config(&mut config_state, &mut warned_config_error);
                    let config = worker_config.config;
                    let async_values = async_values(
                        &cache,
                        &request.state.cwd,
                        &config,
                        worker_config.generation,
                    );
                    let output = render_with_async(&config, &request.state, &async_values);
                    let status = render_status(&config, &async_values);
                    write_record(
                        &mut response,
                        &WorkerRecord::Prompt {
                            generation: request.generation,
                            status,
                            output: output.clone(),
                        },
                    )?;

                    active_prompt = Some(ActivePrompt {
                        generation: request.generation,
                        state: request.state.clone(),
                        config: config.clone(),
                        config_generation: worker_config.generation,
                        output,
                    });
                    schedule_async_refreshes(
                        &job_pool,
                        &mut cache,
                        request.generation,
                        request.state.cwd,
                        &config,
                        worker_config.generation,
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
    config_generation: u64,
    output: LoweredPrompt,
}

#[derive(Clone, Debug, Default)]
struct ConfigState {
    config: Config,
    generation: u64,
}

#[derive(Clone, Debug)]
struct WorkerConfig {
    config: Config,
    generation: u64,
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

    let async_values = async_values(
        cache,
        &active_prompt.state.cwd,
        &active_prompt.config,
        active_prompt.config_generation,
    );
    let output = render_with_async(&active_prompt.config, &active_prompt.state, &async_values);
    if output == active_prompt.output {
        return Ok(());
    }
    let status = render_status(&active_prompt.config, &async_values);

    write_record(
        response,
        &WorkerRecord::Update {
            generation,
            status,
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

fn load_worker_config(
    config_state: &mut ConfigState,
    warned_config_error: &mut bool,
) -> WorkerConfig {
    let config = load_config(None).unwrap_or_else(|error| {
        if !*warned_config_error {
            eprintln!("nova: {error}; using built-in defaults");
            *warned_config_error = true;
        }
        Config::default()
    });

    if config != config_state.config {
        config_state.generation = config_state.generation.saturating_add(1);
        config_state.config = config;
    }

    WorkerConfig {
        config: config_state.config.clone(),
        generation: config_state.generation,
    }
}

fn async_values(
    cache: &SegmentCache,
    cwd: &std::path::Path,
    config: &Config,
    config_generation: u64,
) -> AsyncSegmentValues {
    let now = Instant::now();
    let mut values = AsyncSegmentValues::new();

    if config_uses_any_segment(config, &["git_branch", "git_status"]) {
        let ttl = segment_ttl(&config.segment("git_status"), GIT_REFRESH_TTL);
        let status_key = git_status_key(cwd, config_generation);
        let branch_key = git_branch_key_from_status_key(&status_key);
        values.insert(
            "git_branch".to_string(),
            cache.lookup(&branch_key, now, ttl),
        );
        values.insert(
            "git_status".to_string(),
            cache.lookup(&status_key, now, ttl),
        );
    }

    if config_uses_segment(config, "rust_version") {
        let ttl = segment_ttl(&config.segment("rust_version"), RUNTIME_REFRESH_TTL);
        let key = rust_version_key(cwd, config_generation);
        values.insert("rust_version".to_string(), cache.lookup(&key, now, ttl));
    }

    values
}

fn schedule_async_refreshes(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    cwd: PathBuf,
    config: &Config,
    config_generation: u64,
) {
    schedule_git_refresh(
        pool,
        cache,
        generation,
        cwd.clone(),
        config,
        config_generation,
    );
    schedule_rust_refresh(pool, cache, generation, cwd, config, config_generation);
}

fn schedule_git_refresh(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    cwd: PathBuf,
    config: &Config,
    config_generation: u64,
) {
    if !config_uses_any_segment(config, &["git_branch", "git_status"]) {
        return;
    }

    let status_key = git_status_key(&cwd, config_generation);
    let status_config = config.segment("git_status");
    let ttl = segment_ttl(&status_config, GIT_REFRESH_TTL);
    if !cache.needs_refresh(&status_key, Instant::now(), ttl) {
        return;
    }

    cache.mark_inflight(status_key.clone());
    let branch_key = git_branch_key_from_status_key(&status_key);
    let job_status_key = status_key.clone();
    let job_branch_key = branch_key.clone();
    let branch_config = config.segment("git_branch");
    let timeout = segment_timeout(&status_config, GIT_TIMEOUT);
    let spawn_result = pool.spawn(generation, status_key.clone(), timeout, move |deadline| {
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
    });

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
    config_generation: u64,
) {
    if !config_uses_segment(config, "rust_version") {
        return;
    }

    let key = rust_version_key(&cwd, config_generation);
    let rust_config = config.segment("rust_version");
    let ttl = segment_ttl(&rust_config, RUNTIME_REFRESH_TTL);
    if !cache.needs_refresh(&key, Instant::now(), ttl) {
        return;
    }

    cache.mark_inflight(key.clone());
    let timeout = segment_timeout(&rust_config, RUNTIME_TIMEOUT);
    let job_key = key.clone();
    let spawn_result = pool.spawn(generation, key.clone(), timeout, move |deadline| {
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

fn git_status_key(cwd: &std::path::Path, config_generation: u64) -> CacheKey {
    CacheKey::new("git_status", cwd.to_string_lossy(), config_generation)
}

fn rust_version_key(cwd: &std::path::Path, config_generation: u64) -> CacheKey {
    CacheKey::new("rust_version", cwd.to_string_lossy(), config_generation)
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

fn render_status(config: &Config, async_values: &AsyncSegmentValues) -> RenderStatus {
    let has_incomplete_async_segment = ["git_branch", "git_status", "rust_version"]
        .iter()
        .filter(|id| config_uses_segment(config, id))
        .any(|id| {
            matches!(
                async_values.get(*id),
                None | Some(AsyncValue::Loading | AsyncValue::Stale(_))
            )
        });

    if has_incomplete_async_segment {
        RenderStatus::Partial
    } else {
        RenderStatus::Final
    }
}

fn segment_timeout(config: &SegmentConfig, default: Duration) -> Duration {
    config
        .timeout_ms
        .map(Duration::from_millis)
        .filter(|timeout| !timeout.is_zero())
        .unwrap_or(default)
}

fn segment_ttl(config: &SegmentConfig, default: Duration) -> Duration {
    config.ttl_ms.map(Duration::from_millis).unwrap_or(default)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_timeout_uses_configured_milliseconds() {
        let config = SegmentConfig {
            timeout_ms: Some(2_500),
            ..SegmentConfig::default()
        };

        assert_eq!(
            segment_timeout(&config, Duration::from_secs(1)),
            Duration::from_millis(2_500)
        );
    }

    #[test]
    fn segment_timeout_ignores_zero_overrides() {
        let config = SegmentConfig {
            timeout_ms: Some(0),
            ..SegmentConfig::default()
        };

        assert_eq!(
            segment_timeout(&config, Duration::from_secs(1)),
            Duration::from_secs(1)
        );
    }

    #[test]
    fn segment_ttl_uses_configured_milliseconds() {
        let config = SegmentConfig {
            ttl_ms: Some(2_500),
            ..SegmentConfig::default()
        };

        assert_eq!(
            segment_ttl(&config, Duration::from_secs(1)),
            Duration::from_millis(2_500)
        );
    }

    #[test]
    fn segment_ttl_allows_zero_overrides() {
        let config = SegmentConfig {
            ttl_ms: Some(0),
            ..SegmentConfig::default()
        };

        assert_eq!(segment_ttl(&config, Duration::from_secs(1)), Duration::ZERO);
    }

    #[test]
    fn render_status_is_partial_when_async_values_are_loading_or_stale() {
        let config = Config::default();

        assert_eq!(
            render_status(&config, &AsyncSegmentValues::new()),
            RenderStatus::Partial
        );

        let values = AsyncSegmentValues::from([
            ("git_branch".to_string(), AsyncValue::Failed),
            (
                "git_status".to_string(),
                AsyncValue::Stale(SegmentContent::new(
                    "git_status",
                    "[+1]",
                    Default::default(),
                )),
            ),
            ("rust_version".to_string(), AsyncValue::Failed),
        ]);
        assert_eq!(render_status(&config, &values), RenderStatus::Partial);
    }

    #[test]
    fn render_status_is_final_when_async_values_are_ready_or_failed() {
        let config = Config::default();
        let values = AsyncSegmentValues::from([
            (
                "git_branch".to_string(),
                AsyncValue::Ready(SegmentContent::new(
                    "git_branch",
                    "main",
                    Default::default(),
                )),
            ),
            ("git_status".to_string(), AsyncValue::Failed),
            ("rust_version".to_string(), AsyncValue::Failed),
        ]);

        assert_eq!(render_status(&config, &values), RenderStatus::Final);
    }

    #[test]
    fn render_status_is_final_when_config_has_no_async_segments() {
        let config = Config::from_toml(
            r#"
            [layout]
            lines = 2

            [layout.line1]
            left = ["dir"]
            right = ["duration"]

            [layout.line2]
            left = ["exit_status", "prompt_char"]
            right = []
            "#,
        )
        .expect("sync-only config should parse");

        assert_eq!(
            render_status(&config, &AsyncSegmentValues::new()),
            RenderStatus::Final
        );
    }
}
