//! Per-shell worker event loop.

pub mod jobs;
pub mod protocol;

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::cache::{AsyncValue, CacheKey, SegmentCache};
use crate::config::error::ConfigWarning;
use crate::config::load::load_config;
use crate::config::{Config, DEFAULT_INITIAL_WAIT_MS, SegmentConfig};
use crate::render::{AsyncSegmentValues, LoweredPrompt, render_with_async};
use crate::segments::SegmentContent;
use crate::segments::git::{
    collect_git_status, git_cache_key, render_git_branch, render_git_status,
};
use crate::segments::runtime::{
    bun_cache_key, collect_bun_version, collect_deno_version, collect_node_version,
    collect_python_version, collect_rust_version, deno_cache_key, node_cache_key, python_cache_key,
    render_bun_version, render_deno_version, render_node_version, render_python_version,
    render_rust_version, rust_cache_key,
};
use crate::state::{PromptEnv, PromptState};
use jobs::{JobOutcome, JobPool, JobResult};
use protocol::{
    ClientRecord, FrameDecoder, RenderStatus, WorkerRecord, decode_client_record,
    encode_worker_record,
};

const CACHE_CAPACITY: usize = 128;
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
    let request = OpenOptions::new()
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

    let mut config_state = ConfigState::default();
    let mut warned_config_error = false;
    let mut warned_config_warnings = BTreeSet::new();
    let worker_config = load_worker_config(
        &mut config_state,
        &mut warned_config_error,
        &mut warned_config_warnings,
    );
    write_record(
        &mut response,
        &WorkerRecord::Handshake {
            session_token: options.session_token,
            initial_wait_ms: initial_wait_ms(&worker_config.config),
        },
    )?;

    let (events, event_receiver) = mpsc::channel::<WorkerEvent>();
    spawn_request_reader(request, events.clone());

    let mut decoder = FrameDecoder::default();
    let mut cache = SegmentCache::new(CACHE_CAPACITY);
    let job_pool = JobPool::new(MAX_CONCURRENCY, events);
    let mut active_prompt = None;

    loop {
        match event_receiver.recv() {
            Ok(WorkerEvent::Chunk(chunk)) => {
                handle_request_chunk(
                    &chunk,
                    &mut decoder,
                    &mut config_state,
                    &mut warned_config_error,
                    &mut warned_config_warnings,
                    &mut cache,
                    &mut response,
                    &mut active_prompt,
                    &job_pool,
                )?;
            }
            Ok(WorkerEvent::Job(result)) => {
                handle_job_result(result, &mut cache, &mut response, &mut active_prompt)?;
            }
            Ok(WorkerEvent::Closed) | Err(_) => return Ok(()),
            Ok(WorkerEvent::ReadError(error)) => return Err(WorkerError::Read(error)),
        }
    }
}

#[derive(Debug)]
enum WorkerEvent {
    Chunk(Vec<u8>),
    Closed,
    ReadError(std::io::Error),
    Job(JobResult<AsyncJobSegments>),
}

impl From<JobResult<AsyncJobSegments>> for WorkerEvent {
    fn from(result: JobResult<AsyncJobSegments>) -> Self {
        Self::Job(result)
    }
}

fn spawn_request_reader(mut request: std::fs::File, sender: mpsc::Sender<WorkerEvent>) {
    thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match request.read(&mut buffer) {
                Ok(0) => {
                    let _ = sender.send(WorkerEvent::Closed);
                    return;
                }
                Ok(bytes_read) => {
                    if sender
                        .send(WorkerEvent::Chunk(buffer[..bytes_read].to_vec()))
                        .is_err()
                    {
                        return;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => {
                    let _ = sender.send(WorkerEvent::ReadError(error));
                    return;
                }
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn handle_request_chunk(
    chunk: &[u8],
    decoder: &mut FrameDecoder,
    config_state: &mut ConfigState,
    warned_config_error: &mut bool,
    warned_config_warnings: &mut BTreeSet<ConfigWarning>,
    cache: &mut SegmentCache,
    response: &mut impl Write,
    active_prompt: &mut Option<ActivePrompt>,
    job_pool: &JobPool<AsyncJobSegments>,
) -> Result<(), WorkerError> {
    for frame in decoder.push(chunk) {
        let Ok(ClientRecord::Render(request)) = decode_client_record(&frame) else {
            continue;
        };
        let worker_config =
            load_worker_config(config_state, warned_config_error, warned_config_warnings);
        let config = worker_config.config;
        let async_values = async_values(cache, &request.state, &config, worker_config.generation);
        let output = render_with_async(&config, &request.state, &async_values);
        let status = render_status(&config, &async_values);
        write_record(
            response,
            &WorkerRecord::Prompt {
                generation: request.generation,
                status,
                output: output.clone(),
            },
        )?;

        active_prompt.replace(ActivePrompt {
            generation: request.generation,
            state: request.state.clone(),
            config: config.clone(),
            config_generation: worker_config.generation,
            output,
        });
        schedule_async_refreshes(
            job_pool,
            cache,
            request.generation,
            request.state,
            &config,
            worker_config.generation,
        );
    }

    Ok(())
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
    content: Result<Option<SegmentContent>, CollectFailure>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CollectFailure {
    Failed,
}

fn handle_job_result(
    result: JobResult<AsyncJobSegments>,
    cache: &mut SegmentCache,
    response: &mut impl Write,
    active_prompt: &mut Option<ActivePrompt>,
) -> Result<(), WorkerError> {
    complete_job_result(result, cache);
    write_update_if_active(cache, response, active_prompt)
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
    cache: &SegmentCache,
    response: &mut impl Write,
    active_prompt: &mut Option<ActivePrompt>,
) -> Result<(), WorkerError> {
    let Some(active_prompt) = active_prompt else {
        return Ok(());
    };

    let async_values = async_values(
        cache,
        &active_prompt.state,
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
            generation: active_prompt.generation,
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
    segment: Result<Option<SegmentContent>, CollectFailure>,
    collected_at: Instant,
) {
    match segment {
        Ok(segment) => cache.complete_success(key, segment, collected_at),
        Err(CollectFailure::Failed) => cache.complete_failure(key, collected_at),
    }
}

fn render_collected_version<E>(
    result: Result<Option<String>, E>,
    render: impl FnOnce(&str) -> Option<SegmentContent>,
) -> Result<Option<SegmentContent>, CollectFailure> {
    result
        .map(|version| version.and_then(|version| render(&version)))
        .map_err(|_error| CollectFailure::Failed)
}

fn load_worker_config(
    config_state: &mut ConfigState,
    warned_config_error: &mut bool,
    warned_config_warnings: &mut BTreeSet<ConfigWarning>,
) -> WorkerConfig {
    let config = load_config(None).unwrap_or_else(|error| {
        if !*warned_config_error {
            eprintln!("nova: {error}; using built-in defaults");
            *warned_config_error = true;
        }
        Config::default()
    });
    warn_config_warnings(&config, warned_config_warnings);

    if config != config_state.config {
        config_state.generation = config_state.generation.saturating_add(1);
        config_state.config = config;
    }

    WorkerConfig {
        config: config_state.config.clone(),
        generation: config_state.generation,
    }
}

fn warn_config_warnings(config: &Config, warned_config_warnings: &mut BTreeSet<ConfigWarning>) {
    for warning in config.warnings() {
        if warned_config_warnings.insert(warning.clone()) {
            eprintln!("nova: warning: {warning}");
        }
    }
}

fn async_values(
    cache: &SegmentCache,
    state: &PromptState,
    config: &Config,
    config_generation: u64,
) -> AsyncSegmentValues {
    let now = Instant::now();
    let mut values = AsyncSegmentValues::new();
    let cwd = &state.cwd;
    let path = state.env.path.as_deref();

    if config_uses_any_segment(config, &["git_branch", "git_status"])
        && let Some(status_key) = git_cache_key(cwd, config_generation)
    {
        let ttl = segment_ttl(&config.segment("git_status"), GIT_REFRESH_TTL);
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

    if config_uses_segment(config, "rust_version")
        && let Some(key) = rust_cache_key(cwd, path, config_generation)
    {
        let ttl = segment_ttl(&config.segment("rust_version"), RUNTIME_REFRESH_TTL);
        values.insert("rust_version".to_string(), cache.lookup(&key, now, ttl));
    }

    if config_uses_segment(config, "bun_version")
        && let Some(key) = bun_cache_key(cwd, path, config_generation)
    {
        let ttl = segment_ttl(&config.segment("bun_version"), RUNTIME_REFRESH_TTL);
        values.insert("bun_version".to_string(), cache.lookup(&key, now, ttl));
    }

    if config_uses_segment(config, "deno_version")
        && let Some(key) = deno_cache_key(cwd, path, config_generation)
    {
        let ttl = segment_ttl(&config.segment("deno_version"), RUNTIME_REFRESH_TTL);
        values.insert("deno_version".to_string(), cache.lookup(&key, now, ttl));
    }

    if config_uses_segment(config, "python_version")
        && let Some(key) = python_cache_key(
            cwd,
            state.env.virtual_env.as_deref(),
            path,
            config_generation,
        )
    {
        let ttl = segment_ttl(&config.segment("python_version"), RUNTIME_REFRESH_TTL);
        values.insert("python_version".to_string(), cache.lookup(&key, now, ttl));
    }

    if config_uses_segment(config, "node_version")
        && let Some(key) = node_cache_key(cwd, path, config_generation)
    {
        let ttl = segment_ttl(&config.segment("node_version"), RUNTIME_REFRESH_TTL);
        values.insert("node_version".to_string(), cache.lookup(&key, now, ttl));
    }

    values
}

fn schedule_async_refreshes(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    state: PromptState,
    config: &Config,
    config_generation: u64,
) {
    let cwd = state.cwd;
    let env = state.env;
    let path = env.path.clone();
    schedule_git_refresh(
        pool,
        cache,
        generation,
        cwd.clone(),
        config,
        config_generation,
    );
    schedule_rust_refresh(
        pool,
        cache,
        generation,
        cwd.clone(),
        path.clone(),
        config,
        config_generation,
    );
    schedule_bun_refresh(
        pool,
        cache,
        generation,
        cwd.clone(),
        path.clone(),
        config,
        config_generation,
    );
    schedule_deno_refresh(
        pool,
        cache,
        generation,
        cwd.clone(),
        path.clone(),
        config,
        config_generation,
    );
    schedule_python_refresh(
        pool,
        cache,
        generation,
        cwd.clone(),
        env,
        config,
        config_generation,
    );
    schedule_node_refresh(
        pool,
        cache,
        generation,
        cwd,
        path,
        config,
        config_generation,
    );
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

    let Some(status_key) = git_cache_key(&cwd, config_generation) else {
        return;
    };

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
        let status = match collect_git_status(&cwd, deadline) {
            Ok(Some(status)) => status,
            Ok(None) => {
                return AsyncJobSegments {
                    segments: vec![
                        AsyncJobSegment {
                            key: job_branch_key,
                            content: Ok(None),
                        },
                        AsyncJobSegment {
                            key: job_status_key,
                            content: Ok(None),
                        },
                    ],
                };
            }
            Err(_error) => {
                return AsyncJobSegments {
                    segments: vec![
                        AsyncJobSegment {
                            key: job_branch_key,
                            content: Err(CollectFailure::Failed),
                        },
                        AsyncJobSegment {
                            key: job_status_key,
                            content: Err(CollectFailure::Failed),
                        },
                    ],
                };
            }
        };

        AsyncJobSegments {
            segments: vec![
                AsyncJobSegment {
                    key: job_branch_key,
                    content: Ok(render_git_branch(&status, &branch_config)),
                },
                AsyncJobSegment {
                    key: job_status_key,
                    content: Ok(render_git_status(&status, &status_config)),
                },
            ],
        }
    });

    if spawn_result.is_err() {
        cache.complete_failure(branch_key, Instant::now());
        cache.complete_failure(status_key, Instant::now());
    }
}

fn schedule_rust_refresh(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    cwd: PathBuf,
    path: Option<String>,
    config: &Config,
    config_generation: u64,
) {
    if !config_uses_segment(config, "rust_version") {
        return;
    }

    let Some(key) = rust_cache_key(&cwd, path.as_deref(), config_generation) else {
        return;
    };

    let rust_config = config.segment("rust_version");
    let ttl = segment_ttl(&rust_config, RUNTIME_REFRESH_TTL);
    if !cache.needs_refresh(&key, Instant::now(), ttl) {
        return;
    }

    cache.mark_inflight(key.clone());
    let timeout = segment_timeout(&rust_config, RUNTIME_TIMEOUT);
    let job_key = key.clone();
    let spawn_result = pool.spawn(generation, key.clone(), timeout, move |deadline| {
        let content = render_collected_version(
            collect_rust_version(&cwd, path.as_deref(), deadline),
            |version| render_rust_version(version, &rust_config),
        );
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

fn schedule_node_refresh(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    cwd: PathBuf,
    path: Option<String>,
    config: &Config,
    config_generation: u64,
) {
    if !config_uses_segment(config, "node_version") {
        return;
    }

    let Some(key) = node_cache_key(&cwd, path.as_deref(), config_generation) else {
        return;
    };

    let node_config = config.segment("node_version");
    let ttl = segment_ttl(&node_config, RUNTIME_REFRESH_TTL);
    if !cache.needs_refresh(&key, Instant::now(), ttl) {
        return;
    }

    cache.mark_inflight(key.clone());
    let timeout = segment_timeout(&node_config, RUNTIME_TIMEOUT);
    let job_key = key.clone();
    let spawn_result = pool.spawn(generation, key.clone(), timeout, move |deadline| {
        let content = render_collected_version(
            collect_node_version(&cwd, path.as_deref(), deadline),
            |version| render_node_version(version, &node_config),
        );
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

fn schedule_bun_refresh(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    cwd: PathBuf,
    path: Option<String>,
    config: &Config,
    config_generation: u64,
) {
    if !config_uses_segment(config, "bun_version") {
        return;
    }

    let Some(key) = bun_cache_key(&cwd, path.as_deref(), config_generation) else {
        return;
    };

    let bun_config = config.segment("bun_version");
    let ttl = segment_ttl(&bun_config, RUNTIME_REFRESH_TTL);
    if !cache.needs_refresh(&key, Instant::now(), ttl) {
        return;
    }

    cache.mark_inflight(key.clone());
    let timeout = segment_timeout(&bun_config, RUNTIME_TIMEOUT);
    let job_key = key.clone();
    let spawn_result = pool.spawn(generation, key.clone(), timeout, move |deadline| {
        let content = render_collected_version(
            collect_bun_version(&cwd, path.as_deref(), deadline),
            |version| render_bun_version(version, &bun_config),
        );
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

fn schedule_deno_refresh(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    cwd: PathBuf,
    path: Option<String>,
    config: &Config,
    config_generation: u64,
) {
    if !config_uses_segment(config, "deno_version") {
        return;
    }

    let Some(key) = deno_cache_key(&cwd, path.as_deref(), config_generation) else {
        return;
    };

    let deno_config = config.segment("deno_version");
    let ttl = segment_ttl(&deno_config, RUNTIME_REFRESH_TTL);
    if !cache.needs_refresh(&key, Instant::now(), ttl) {
        return;
    }

    cache.mark_inflight(key.clone());
    let timeout = segment_timeout(&deno_config, RUNTIME_TIMEOUT);
    let job_key = key.clone();
    let spawn_result = pool.spawn(generation, key.clone(), timeout, move |deadline| {
        let content = render_collected_version(
            collect_deno_version(&cwd, path.as_deref(), deadline),
            |version| render_deno_version(version, &deno_config),
        );
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

fn schedule_python_refresh(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    cwd: PathBuf,
    env: PromptEnv,
    config: &Config,
    config_generation: u64,
) {
    if !config_uses_segment(config, "python_version") {
        return;
    }

    let Some(key) = python_cache_key(
        &cwd,
        env.virtual_env.as_deref(),
        env.path.as_deref(),
        config_generation,
    ) else {
        return;
    };

    let python_config = config.segment("python_version");
    let ttl = segment_ttl(&python_config, RUNTIME_REFRESH_TTL);
    if !cache.needs_refresh(&key, Instant::now(), ttl) {
        return;
    }

    cache.mark_inflight(key.clone());
    let timeout = segment_timeout(&python_config, RUNTIME_TIMEOUT);
    let job_key = key.clone();
    let spawn_result = pool.spawn(generation, key.clone(), timeout, move |deadline| {
        let content = render_collected_version(
            collect_python_version(
                &cwd,
                env.virtual_env.as_deref(),
                env.path.as_deref(),
                deadline,
            ),
            |version| render_python_version(version, &python_config),
        );
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
    let has_incomplete_async_segment = async_values
        .iter()
        .filter(|(id, _value)| config_uses_segment(config, id))
        .any(|(_id, value)| matches!(value, AsyncValue::Loading));

    if has_incomplete_async_segment {
        RenderStatus::Partial
    } else {
        RenderStatus::Final
    }
}

fn initial_wait_ms(config: &Config) -> u64 {
    config
        .async_config
        .initial_wait_ms
        .unwrap_or(DEFAULT_INITIAL_WAIT_MS)
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
    use crate::worker::protocol::decode_worker_record;

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
    fn render_status_is_final_when_no_async_values_are_applicable() {
        let config = Config::default();

        assert_eq!(
            render_status(&config, &AsyncSegmentValues::new()),
            RenderStatus::Final
        );
    }

    #[test]
    fn render_status_is_partial_when_async_values_are_loading() {
        let config = Config::default();

        let values = AsyncSegmentValues::from([
            ("git_branch".to_string(), AsyncValue::Failed),
            ("git_status".to_string(), AsyncValue::Loading),
            ("rust_version".to_string(), AsyncValue::Failed),
        ]);
        assert_eq!(render_status(&config, &values), RenderStatus::Partial);
    }

    #[test]
    fn render_status_is_final_when_async_values_are_stale() {
        let config = Config::default();

        let values = AsyncSegmentValues::from([
            ("git_branch".to_string(), AsyncValue::Failed),
            (
                "git_status".to_string(),
                AsyncValue::Stale(Some(SegmentContent::new(
                    "git_status",
                    "[+1]",
                    Default::default(),
                ))),
            ),
            ("rust_version".to_string(), AsyncValue::Failed),
        ]);
        assert_eq!(render_status(&config, &values), RenderStatus::Final);
    }

    #[test]
    fn render_status_is_final_when_async_values_are_ready_or_failed() {
        let config = Config::default();
        let values = AsyncSegmentValues::from([
            (
                "git_branch".to_string(),
                AsyncValue::Ready(Some(SegmentContent::new(
                    "git_branch",
                    "main",
                    Default::default(),
                ))),
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

    #[test]
    fn stale_generation_job_updates_active_prompt_generation() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        std::fs::write(
            tempdir.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("Cargo.toml should be written");
        let config = Config::from_toml(
            r#"
            [layout]
            lines = 2

            [layout.line1]
            left = ["rust_version"]
            right = []

            [layout.line2]
            left = ["prompt_char"]
            right = []
            "#,
        )
        .expect("config should parse");
        let config_generation = 1;
        let state = PromptState {
            cwd: tempdir.path().to_path_buf(),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 120,
            keymap: Default::default(),
            env: Default::default(),
        };
        let key = rust_cache_key(&state.cwd, None, config_generation)
            .expect("rust cache key should be available");
        let mut cache = SegmentCache::new(4);
        let initial_values = async_values(&cache, &state, &config, config_generation);
        let output = render_with_async(&config, &state, &initial_values);
        assert!(!output.prompt.contains("1.96.1"));

        let mut active_prompt = Some(ActivePrompt {
            generation: 2,
            state,
            config,
            config_generation,
            output,
        });
        let finished_at = Instant::now();
        let result = JobResult {
            generation: 1,
            key: key.clone(),
            started_at: finished_at,
            finished_at,
            outcome: JobOutcome::Completed(AsyncJobSegments {
                segments: vec![AsyncJobSegment {
                    key,
                    content: Ok(Some(SegmentContent::new(
                        "rust_version",
                        "1.96.1",
                        Default::default(),
                    ))),
                }],
            }),
        };
        let mut response = Vec::new();

        handle_job_result(result, &mut cache, &mut response, &mut active_prompt)
            .expect("job result should be handled");

        let encoded = String::from_utf8(response).expect("response should be utf8");
        let frame = encoded.trim_end_matches('\x1e');
        let WorkerRecord::Update {
            generation,
            status,
            output,
        } = decode_worker_record(frame).expect("update should decode")
        else {
            panic!("expected update response");
        };

        assert_eq!(generation, 2);
        assert_eq!(status, RenderStatus::Final);
        assert!(output.prompt.contains("1.96.1"));
    }

    #[test]
    fn final_status_still_allows_later_updates() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        std::fs::write(
            tempdir.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("Cargo.toml should be written");
        let config = Config::from_toml(
            r#"
            [layout]
            lines = 2

            [layout.line1]
            left = ["rust_version"]
            right = []

            [layout.line2]
            left = ["prompt_char"]
            right = []

            [segments.rust_version]
            ttl_ms = 0
            "#,
        )
        .expect("config should parse");
        let config_generation = 1;
        let state = PromptState {
            cwd: tempdir.path().to_path_buf(),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 120,
            keymap: Default::default(),
            env: Default::default(),
        };
        let key = rust_cache_key(&state.cwd, None, config_generation)
            .expect("rust cache key should be available");
        let mut cache = SegmentCache::new(4);
        cache.complete_success(
            key.clone(),
            Some(SegmentContent::new(
                "rust_version",
                "1.95.0",
                Default::default(),
            )),
            Instant::now(),
        );

        let initial_values = async_values(&cache, &state, &config, config_generation);
        let output = render_with_async(&config, &state, &initial_values);
        assert_eq!(render_status(&config, &initial_values), RenderStatus::Final);
        assert!(output.prompt.contains("1.95.0"));

        let mut active_prompt = Some(ActivePrompt {
            generation: 2,
            state,
            config,
            config_generation,
            output,
        });
        let finished_at = Instant::now();
        let result = JobResult {
            generation: 2,
            key: key.clone(),
            started_at: finished_at,
            finished_at,
            outcome: JobOutcome::Completed(AsyncJobSegments {
                segments: vec![AsyncJobSegment {
                    key,
                    content: Ok(Some(SegmentContent::new(
                        "rust_version",
                        "1.96.1",
                        Default::default(),
                    ))),
                }],
            }),
        };
        let mut response = Vec::new();

        handle_job_result(result, &mut cache, &mut response, &mut active_prompt)
            .expect("job result should be handled");

        let encoded = String::from_utf8(response).expect("response should be utf8");
        let frame = encoded.trim_end_matches('\x1e');
        let WorkerRecord::Update {
            generation,
            status,
            output,
        } = decode_worker_record(frame).expect("update should decode")
        else {
            panic!("expected update response");
        };

        assert_eq!(generation, 2);
        assert_eq!(status, RenderStatus::Final);
        assert!(output.prompt.contains("1.96.1"));
    }

    #[test]
    fn empty_job_result_updates_cache_without_changing_output() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        std::fs::write(
            tempdir.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("Cargo.toml should be written");
        let config = Config::from_toml(
            r#"
            [layout]
            lines = 2

            [layout.line1]
            left = ["rust_version"]
            right = []

            [layout.line2]
            left = ["prompt_char"]
            right = []
            "#,
        )
        .expect("config should parse");
        let config_generation = 1;
        let state = PromptState {
            cwd: tempdir.path().to_path_buf(),
            exit_status: 0,
            duration_ms: None,
            time: None,
            columns: 120,
            keymap: Default::default(),
            env: Default::default(),
        };
        let key = rust_cache_key(&state.cwd, None, config_generation)
            .expect("rust cache key should be available");
        let mut cache = SegmentCache::new(4);
        let initial_values = async_values(&cache, &state, &config, config_generation);
        let output = render_with_async(&config, &state, &initial_values);

        let mut active_prompt = Some(ActivePrompt {
            generation: 2,
            state,
            config,
            config_generation,
            output: output.clone(),
        });
        let finished_at = Instant::now();
        let result = JobResult {
            generation: 1,
            key: key.clone(),
            started_at: finished_at,
            finished_at,
            outcome: JobOutcome::Completed(AsyncJobSegments {
                segments: vec![AsyncJobSegment {
                    key,
                    content: Ok(None),
                }],
            }),
        };
        let mut response = Vec::new();

        handle_job_result(result, &mut cache, &mut response, &mut active_prompt)
            .expect("job result should be handled");

        assert!(response.is_empty());
        let active_prompt = active_prompt.expect("active prompt should remain");
        assert_eq!(active_prompt.output, output);
        let async_values = async_values(
            &cache,
            &active_prompt.state,
            &active_prompt.config,
            active_prompt.config_generation,
        );
        assert_eq!(
            async_values.get("rust_version"),
            Some(&AsyncValue::Ready(None))
        );
    }
}
