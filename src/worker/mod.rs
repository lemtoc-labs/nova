//! Per-shell worker event loop.

pub mod jobs;
pub mod protocol;

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::cache::{AsyncValue, CacheKey, SegmentCache};
use crate::config::error::ConfigWarning;
use crate::config::load::{ConfigSnapshot, ConfigSource};
use crate::config::{Config, DEFAULT_INITIAL_WAIT_MS, DEFAULT_MIN_LOADING_MS, SegmentConfig};
use crate::render::{AsyncSegmentValues, LoweredPrompt, render_with_async};
use crate::segments::{
    ASYNC_SEGMENTS, AsyncJobSegment, AsyncSegmentFailure, CollectContext, SegmentContent,
    known_segment_ids,
};
use crate::state::PromptState;
use jobs::{JobOutcome, JobPool, JobResult};
use protocol::{
    ClientRecord, FrameDecoder, RenderStatus, WorkerRecord, decode_client_record,
    encode_worker_record,
};

const CACHE_CAPACITY: usize = 128;
const MAX_CONCURRENCY: usize = 2;
const PARENT_CHECK_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug)]
pub struct WorkerOptions {
    pub runtime_dir: PathBuf,
    pub session_token: String,
    pub parent_pid: Option<u32>,
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
    let _runtime_cleanup = RuntimeDirCleanup::new(options.runtime_dir.clone());
    spawn_parent_monitor(options.parent_pid, options.runtime_dir.clone());

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
    let job_pool = JobPool::new(MAX_CONCURRENCY, events.clone());
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
                handle_job_result(
                    result,
                    &mut cache,
                    &mut response,
                    &mut active_prompt,
                    &events,
                )?;
            }
            Ok(WorkerEvent::Closed) | Err(_) => return Ok(()),
            Ok(WorkerEvent::ReadError(error)) => return Err(WorkerError::Read(error)),
        }
    }
}

#[derive(Debug)]
struct RuntimeDirCleanup {
    path: PathBuf,
}

impl RuntimeDirCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for RuntimeDirCleanup {
    fn drop(&mut self) {
        cleanup_runtime_dir(&self.path);
    }
}

fn spawn_parent_monitor(parent_pid: Option<u32>, runtime_dir: PathBuf) {
    let Some(parent_pid) = parent_pid else {
        return;
    };

    thread::spawn(move || {
        loop {
            thread::sleep(PARENT_CHECK_INTERVAL);
            if !parent_is_alive(parent_pid) {
                cleanup_runtime_dir(&runtime_dir);
                std::process::exit(0);
            }
        }
    });
}

fn parent_is_alive(parent_pid: u32) -> bool {
    for command in ["/bin/kill", "/usr/bin/kill", "kill"] {
        match Command::new(command)
            .arg("-0")
            .arg(parent_pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Ok(status) => return status.success(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return true,
        }
    }

    true
}

fn cleanup_runtime_dir(path: &Path) {
    if is_nova_runtime_dir(path) {
        let _ = std::fs::remove_dir_all(path);
    }
}

fn is_nova_runtime_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("nova-"))
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
            config: Arc::clone(&config),
            config_generation: worker_config.generation,
            output,
        });
        schedule_async_refreshes(
            job_pool,
            cache,
            request.generation,
            request.state,
            Arc::clone(&config),
            worker_config.generation,
        );
    }

    Ok(())
}

#[derive(Clone, Debug)]
struct ActivePrompt {
    generation: u64,
    state: PromptState,
    config: Arc<Config>,
    config_generation: u64,
    output: LoweredPrompt,
}

#[derive(Clone, Debug)]
struct ConfigState {
    source: ConfigSource,
    snapshot: Option<ConfigSnapshot>,
    config: Arc<Config>,
    generation: u64,
}

impl Default for ConfigState {
    fn default() -> Self {
        Self {
            source: ConfigSource::discover(),
            snapshot: None,
            config: Arc::new(Config::default()),
            generation: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct WorkerConfig {
    config: Arc<Config>,
    generation: u64,
}

#[derive(Debug)]
struct AsyncJobSegments {
    min_loading_until: Instant,
    segments: Vec<AsyncJobSegment>,
}

fn handle_job_result(
    result: JobResult<AsyncJobSegments>,
    cache: &mut SegmentCache,
    response: &mut impl Write,
    active_prompt: &mut Option<ActivePrompt>,
    events: &mpsc::Sender<WorkerEvent>,
) -> Result<(), WorkerError> {
    if let Some(delay) = job_result_delay(&result, Instant::now()) {
        defer_job_result(result, delay, events.clone());
        return Ok(());
    }

    complete_job_result(result, cache);
    write_update_if_active(cache, response, active_prompt)
}

fn job_result_delay(result: &JobResult<AsyncJobSegments>, now: Instant) -> Option<Duration> {
    let JobOutcome::Completed(segments) = &result.outcome else {
        return None;
    };

    if segments.min_loading_until <= now {
        return None;
    }

    Some(segments.min_loading_until.duration_since(now))
}

fn defer_job_result(
    result: JobResult<AsyncJobSegments>,
    delay: Duration,
    events: mpsc::Sender<WorkerEvent>,
) {
    thread::spawn(move || {
        thread::sleep(delay);
        let _ = events.send(WorkerEvent::Job(result));
    });
}

fn complete_job_result(result: JobResult<AsyncJobSegments>, cache: &mut SegmentCache) {
    match result.outcome {
        JobOutcome::Completed(segments) => {
            for segment in segments.segments {
                complete_segment(cache, segment.key, segment.content, result.finished_at);
            }
        }
        JobOutcome::Panicked => {
            let mut fail_keys = result.fail_keys;
            if !fail_keys.contains(&result.key) {
                fail_keys.push(result.key);
            }
            for key in fail_keys {
                cache.complete_failure(key, result.finished_at);
            }
        }
    }
}

fn write_update_if_active(
    cache: &mut SegmentCache,
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
    segment: Result<Option<SegmentContent>, AsyncSegmentFailure>,
    collected_at: Instant,
) {
    match segment {
        Ok(segment) => cache.complete_success(key, segment, collected_at),
        Err(AsyncSegmentFailure::Failed) => cache.complete_failure(key, collected_at),
    }
}

fn load_worker_config(
    config_state: &mut ConfigState,
    warned_config_error: &mut bool,
    warned_config_warnings: &mut BTreeSet<ConfigWarning>,
) -> WorkerConfig {
    let snapshot = match config_state.source.snapshot() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if !*warned_config_error {
                eprintln!("nova: {error}; using built-in defaults");
                *warned_config_error = true;
            }
            let config = Config::default();
            if *config_state.config != config {
                config_state.generation = config_state.generation.saturating_add(1);
                config_state.config = Arc::new(config);
            }
            return WorkerConfig {
                config: Arc::clone(&config_state.config),
                generation: config_state.generation,
            };
        }
    };

    if config_state.snapshot.as_ref() == Some(&snapshot) {
        return WorkerConfig {
            config: Arc::clone(&config_state.config),
            generation: config_state.generation,
        };
    }

    let config = config_state
        .source
        .load_snapshot(&snapshot)
        .unwrap_or_else(|error| {
            if !*warned_config_error {
                eprintln!("nova: {error}; using built-in defaults");
                *warned_config_error = true;
            }
            Config::default()
        });
    warn_config_warnings(&config, warned_config_warnings);

    if *config_state.config != config {
        config_state.generation = config_state.generation.saturating_add(1);
        config_state.config = Arc::new(config);
    }
    config_state.snapshot = Some(snapshot);

    WorkerConfig {
        config: Arc::clone(&config_state.config),
        generation: config_state.generation,
    }
}

fn warn_config_warnings(config: &Config, warned_config_warnings: &mut BTreeSet<ConfigWarning>) {
    for warning in config.warnings(known_segment_ids()) {
        if warned_config_warnings.insert(warning.clone()) {
            eprintln!("nova: warning: {warning}");
        }
    }
}

fn async_values(
    cache: &mut SegmentCache,
    state: &PromptState,
    config: &Config,
    config_generation: u64,
) -> AsyncSegmentValues {
    let now = Instant::now();
    let mut values = AsyncSegmentValues::new();

    for segment in ASYNC_SEGMENTS {
        if !config_uses_any_segment(config, segment.render_ids()) {
            continue;
        }

        let ttl = segment_ttl(config.segment(segment.primary_id()), segment.default_ttl());
        for render_id in segment.render_ids() {
            let Some(key) = segment.cache_key(render_id, state, config_generation) else {
                continue;
            };
            values.insert((*render_id).to_string(), cache.lookup(&key, now, ttl));
        }
    }

    values
}

fn schedule_async_refreshes(
    pool: &JobPool<AsyncJobSegments>,
    cache: &mut SegmentCache,
    generation: u64,
    state: PromptState,
    config: Arc<Config>,
    config_generation: u64,
) {
    for segment in ASYNC_SEGMENTS {
        if !config_uses_any_segment(&config, segment.render_ids()) {
            continue;
        }

        let Some(key) = segment.cache_key(segment.primary_id(), &state, config_generation) else {
            continue;
        };
        let now = Instant::now();
        let segment_config = config.segment(segment.primary_id());
        let ttl = segment_ttl(segment_config, segment.default_ttl());
        if !cache.needs_refresh(&key, now, ttl) {
            continue;
        }

        let min_loading_until =
            segment_min_loading_until(cache, &key, &config, segment.primary_id(), now);
        let fail_keys = segment
            .render_ids()
            .iter()
            .filter_map(|render_id| segment.cache_key(render_id, &state, config_generation))
            .collect::<Vec<_>>();
        cache.mark_inflight(key.clone());
        let timeout = segment_timeout(segment_config, segment.default_timeout());
        let job_state = state.clone();
        let job_config = Arc::clone(&config);
        let spawn_result = pool.spawn_with_fail_keys(
            generation,
            key.clone(),
            fail_keys.clone(),
            timeout,
            move |deadline| AsyncJobSegments {
                min_loading_until,
                segments: segment.collect(&CollectContext {
                    state: &job_state,
                    config: &job_config,
                    config_generation,
                    deadline,
                }),
            },
        );

        if spawn_result.is_err() {
            let finished_at = Instant::now();
            for key in fail_keys {
                cache.complete_failure(key, finished_at);
            }
        }
    }
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

fn segment_min_loading(config: &Config, segment_config: &SegmentConfig) -> Duration {
    segment_config
        .min_loading_ms
        .or(config.async_config.min_loading_ms)
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_millis(DEFAULT_MIN_LOADING_MS))
}

fn segment_min_loading_until(
    cache: &SegmentCache,
    key: &CacheKey,
    config: &Config,
    segment_id: &str,
    now: Instant,
) -> Instant {
    if cache.has_entry(key) {
        return now;
    }

    now + segment_min_loading(config, config.segment(segment_id))
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
    use crate::segments::runtime::rust_cache_key;
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
    fn segment_min_loading_uses_global_config() {
        let config = Config {
            async_config: crate::config::AsyncConfig {
                min_loading_ms: Some(250),
                ..Default::default()
            },
            ..Config::default()
        };

        assert_eq!(
            segment_min_loading(&config, &SegmentConfig::default()),
            Duration::from_millis(250)
        );
    }

    #[test]
    fn segment_min_loading_uses_builtin_default() {
        assert_eq!(
            segment_min_loading(&Config::default(), &SegmentConfig::default()),
            Duration::from_millis(DEFAULT_MIN_LOADING_MS)
        );
    }

    #[test]
    fn segment_min_loading_prefers_segment_config() {
        let config = Config {
            async_config: crate::config::AsyncConfig {
                min_loading_ms: Some(250),
                ..Default::default()
            },
            ..Config::default()
        };
        let segment_config = SegmentConfig {
            min_loading_ms: Some(75),
            ..SegmentConfig::default()
        };

        assert_eq!(
            segment_min_loading(&config, &segment_config),
            Duration::from_millis(75)
        );
    }

    #[test]
    fn segment_min_loading_allows_zero_segment_override() {
        let config = Config {
            async_config: crate::config::AsyncConfig {
                min_loading_ms: Some(250),
                ..Default::default()
            },
            ..Config::default()
        };
        let segment_config = SegmentConfig {
            min_loading_ms: Some(0),
            ..SegmentConfig::default()
        };

        assert_eq!(
            segment_min_loading(&config, &segment_config),
            Duration::ZERO
        );
    }

    #[test]
    fn segment_min_loading_until_waits_for_missing_cache_entry() {
        let cache = SegmentCache::new(4);
        let config = Config {
            async_config: crate::config::AsyncConfig {
                min_loading_ms: Some(40),
                ..Default::default()
            },
            ..Config::default()
        };
        let now = Instant::now();
        let key = CacheKey::new("rust_version", "/repo", 1);

        assert_eq!(
            segment_min_loading_until(&cache, &key, &config, "rust_version", now),
            now + Duration::from_millis(40)
        );
    }

    #[test]
    fn segment_min_loading_until_does_not_wait_for_cached_entry() {
        let mut cache = SegmentCache::new(4);
        let config = Config {
            async_config: crate::config::AsyncConfig {
                min_loading_ms: Some(40),
                ..Default::default()
            },
            ..Config::default()
        };
        let now = Instant::now();
        let key = CacheKey::new("rust_version", "/repo", 1);
        cache.complete_success(
            key.clone(),
            Some(SegmentContent::new(
                "rust_version",
                "1.96.1",
                Default::default(),
            )),
            now - Duration::from_secs(60),
        );

        assert_eq!(
            segment_min_loading_until(&cache, &key, &config, "rust_version", now),
            now
        );
    }

    #[test]
    fn job_result_delay_waits_until_min_loading_deadline() {
        let now = Instant::now();
        let key = CacheKey::new("rust_version", "/repo", 1);
        let result = JobResult {
            generation: 1,
            key: key.clone(),
            fail_keys: vec![key],
            started_at: now,
            finished_at: now,
            outcome: JobOutcome::Completed(AsyncJobSegments {
                min_loading_until: now + Duration::from_millis(10),
                segments: Vec::new(),
            }),
        };

        assert_eq!(
            job_result_delay(&result, now),
            Some(Duration::from_millis(10))
        );
    }

    #[test]
    fn job_result_delay_only_waits_for_remaining_min_loading_time() {
        let started_at = Instant::now();
        let finished_at = started_at + Duration::from_millis(6);
        let key = CacheKey::new("rust_version", "/repo", 1);
        let result = JobResult {
            generation: 1,
            key: key.clone(),
            fail_keys: vec![key],
            started_at,
            finished_at,
            outcome: JobOutcome::Completed(AsyncJobSegments {
                min_loading_until: started_at + Duration::from_millis(10),
                segments: Vec::new(),
            }),
        };

        assert_eq!(
            job_result_delay(&result, finished_at),
            Some(Duration::from_millis(4))
        );
    }

    #[test]
    fn job_result_delay_is_none_when_job_exceeds_min_loading_time() {
        let started_at = Instant::now();
        let finished_at = started_at + Duration::from_millis(15);
        let key = CacheKey::new("rust_version", "/repo", 1);
        let result = JobResult {
            generation: 1,
            key: key.clone(),
            fail_keys: vec![key],
            started_at,
            finished_at,
            outcome: JobOutcome::Completed(AsyncJobSegments {
                min_loading_until: started_at + Duration::from_millis(10),
                segments: Vec::new(),
            }),
        };

        assert_eq!(job_result_delay(&result, finished_at), None);
    }

    #[test]
    fn job_result_delay_is_none_after_min_loading_deadline() {
        let now = Instant::now();
        let key = CacheKey::new("rust_version", "/repo", 1);
        let result = JobResult {
            generation: 1,
            key: key.clone(),
            fail_keys: vec![key],
            started_at: now,
            finished_at: now,
            outcome: JobOutcome::Completed(AsyncJobSegments {
                min_loading_until: now,
                segments: Vec::new(),
            }),
        };

        assert_eq!(job_result_delay(&result, now), None);
    }

    #[test]
    fn job_result_delay_is_none_for_panicked_jobs() {
        let now = Instant::now();
        let key = CacheKey::new("rust_version", "/repo", 1);
        let result = JobResult {
            generation: 1,
            key: key.clone(),
            fail_keys: vec![key],
            started_at: now,
            finished_at: now,
            outcome: JobOutcome::Panicked,
        };

        assert_eq!(job_result_delay(&result, now), None);
    }

    #[test]
    fn handle_job_result_defers_completion_until_min_loading_deadline() {
        let now = Instant::now();
        let key = CacheKey::new("rust_version", "/repo", 1);
        let result = JobResult {
            generation: 1,
            key: key.clone(),
            fail_keys: vec![key.clone()],
            started_at: now,
            finished_at: now,
            outcome: JobOutcome::Completed(AsyncJobSegments {
                min_loading_until: now + Duration::from_millis(5),
                segments: vec![AsyncJobSegment {
                    key: key.clone(),
                    content: Ok(Some(SegmentContent::new(
                        "rust_version",
                        "1.96.1",
                        Default::default(),
                    ))),
                }],
            }),
        };
        let mut cache = SegmentCache::new(4);
        let mut response = Vec::new();
        let mut active_prompt = None;
        let (events, event_receiver) = mpsc::channel();

        handle_job_result(
            result,
            &mut cache,
            &mut response,
            &mut active_prompt,
            &events,
        )
        .expect("job result should be deferred");

        assert!(response.is_empty());
        assert_eq!(
            cache.lookup(&key, now, Duration::from_secs(1)),
            AsyncValue::Loading
        );
        let event = event_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("deferred job result should be requeued");
        let WorkerEvent::Job(deferred) = event else {
            panic!("expected deferred job result");
        };
        assert_eq!(deferred.key, key);
    }

    #[test]
    fn load_worker_config_reuses_config_when_snapshot_is_unchanged() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let path = tempdir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
            [layout]
            lines = 1
            "#,
        )
        .expect("config should be written");
        let mut config_state = ConfigState {
            source: ConfigSource::from_path(Some(path)),
            ..ConfigState::default()
        };
        let mut warned_config_error = false;
        let mut warned_config_warnings = BTreeSet::new();

        let first = load_worker_config(
            &mut config_state,
            &mut warned_config_error,
            &mut warned_config_warnings,
        );
        let second = load_worker_config(
            &mut config_state,
            &mut warned_config_error,
            &mut warned_config_warnings,
        );

        assert_eq!(first.generation, 1);
        assert_eq!(second.generation, 1);
        assert!(Arc::ptr_eq(&first.config, &second.config));
    }

    #[test]
    fn load_worker_config_tracks_file_appearance_and_deletion() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let path = tempdir.path().join("config.toml");
        let mut config_state = ConfigState {
            source: ConfigSource::from_path(Some(path.clone())),
            ..ConfigState::default()
        };
        let mut warned_config_error = false;
        let mut warned_config_warnings = BTreeSet::new();

        let missing = load_worker_config(
            &mut config_state,
            &mut warned_config_error,
            &mut warned_config_warnings,
        );
        assert_eq!(missing.generation, 0);
        assert_eq!(missing.config.layout.lines, 2);

        std::fs::write(
            &path,
            r#"
            [layout]
            lines = 1
            "#,
        )
        .expect("config should be written");
        let appeared = load_worker_config(
            &mut config_state,
            &mut warned_config_error,
            &mut warned_config_warnings,
        );
        assert_eq!(appeared.generation, 1);
        assert_eq!(appeared.config.layout.lines, 1);

        std::fs::remove_file(&path).expect("config should be removed");
        let deleted = load_worker_config(
            &mut config_state,
            &mut warned_config_error,
            &mut warned_config_warnings,
        );
        assert_eq!(deleted.generation, 2);
        assert_eq!(deleted.config.layout.lines, 2);
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
        let initial_values = async_values(&mut cache, &state, &config, config_generation);
        let output = render_with_async(&config, &state, &initial_values);
        assert!(!output.prompt.contains("1.96.1"));

        let mut active_prompt = Some(ActivePrompt {
            generation: 2,
            state,
            config: Arc::new(config),
            config_generation,
            output,
        });
        let finished_at = Instant::now();
        let result = JobResult {
            generation: 1,
            key: key.clone(),
            fail_keys: vec![key.clone()],
            started_at: finished_at,
            finished_at,
            outcome: JobOutcome::Completed(AsyncJobSegments {
                min_loading_until: finished_at,
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
        let (events, _event_receiver) = mpsc::channel();

        handle_job_result(
            result,
            &mut cache,
            &mut response,
            &mut active_prompt,
            &events,
        )
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

        let initial_values = async_values(&mut cache, &state, &config, config_generation);
        let output = render_with_async(&config, &state, &initial_values);
        assert_eq!(render_status(&config, &initial_values), RenderStatus::Final);
        assert!(output.prompt.contains("1.95.0"));

        let mut active_prompt = Some(ActivePrompt {
            generation: 2,
            state,
            config: Arc::new(config),
            config_generation,
            output,
        });
        let finished_at = Instant::now();
        let result = JobResult {
            generation: 2,
            key: key.clone(),
            fail_keys: vec![key.clone()],
            started_at: finished_at,
            finished_at,
            outcome: JobOutcome::Completed(AsyncJobSegments {
                min_loading_until: finished_at,
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
        let (events, _event_receiver) = mpsc::channel();

        handle_job_result(
            result,
            &mut cache,
            &mut response,
            &mut active_prompt,
            &events,
        )
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
        let initial_values = async_values(&mut cache, &state, &config, config_generation);
        let output = render_with_async(&config, &state, &initial_values);

        let mut active_prompt = Some(ActivePrompt {
            generation: 2,
            state,
            config: Arc::new(config),
            config_generation,
            output: output.clone(),
        });
        let finished_at = Instant::now();
        let result = JobResult {
            generation: 1,
            key: key.clone(),
            fail_keys: vec![key.clone()],
            started_at: finished_at,
            finished_at,
            outcome: JobOutcome::Completed(AsyncJobSegments {
                min_loading_until: finished_at,
                segments: vec![AsyncJobSegment {
                    key,
                    content: Ok(None),
                }],
            }),
        };
        let mut response = Vec::new();
        let (events, _event_receiver) = mpsc::channel();

        handle_job_result(
            result,
            &mut cache,
            &mut response,
            &mut active_prompt,
            &events,
        )
        .expect("job result should be handled");

        assert!(response.is_empty());
        let active_prompt = active_prompt.expect("active prompt should remain");
        assert_eq!(active_prompt.output, output);
        let async_values = async_values(
            &mut cache,
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
