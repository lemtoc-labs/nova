use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use nova::cache::AsyncValue;
use nova::config::Config;
use nova::render::{AsyncSegmentValues, render, render_with_async};
use nova::segments::{SegmentContent, Style};
use nova::state::{AwsEnv, Keymap, PromptEnv, PromptState};

const WARMUP_ITERATIONS: usize = 10_000;
const MEASURE_ITERATIONS: usize = 200_000;

fn main() {
    let config = Config::default();
    let state = prompt_state(PathBuf::from("/Users/example/dev/oss/nova"), 120);
    let narrow_state = prompt_state(
        PathBuf::from("/Users/example/dev/oss/nova/crates/nova/submodule"),
        40,
    );
    let async_values = async_values();

    bench("render_default_sync", || render(&config, &state));
    bench("render_default_cached_async", || {
        render_with_async(&config, &state, &async_values)
    });
    bench("render_narrow_cached_async", || {
        render_with_async(&config, &narrow_state, &async_values)
    });
}

fn bench<T>(name: &str, mut render_once: impl FnMut() -> T) {
    for _ in 0..WARMUP_ITERATIONS {
        black_box(render_once());
    }

    let started_at = Instant::now();
    for _ in 0..MEASURE_ITERATIONS {
        black_box(render_once());
    }
    report(name, started_at.elapsed());
}

fn report(name: &str, elapsed: Duration) {
    let total_ns = elapsed.as_nanos();
    let mean_ns = total_ns / MEASURE_ITERATIONS as u128;
    let mean_us = mean_ns as f64 / 1_000.0;
    println!(
        "{name}: iterations={MEASURE_ITERATIONS} total_ms={:.3} mean_us={mean_us:.3}",
        total_ns as f64 / 1_000_000.0
    );
}

fn prompt_state(cwd: PathBuf, columns: u16) -> PromptState {
    PromptState {
        cwd,
        exit_status: 0,
        duration_ms: Some(12_345),
        time: Some("11:16:42".to_string()),
        columns,
        keymap: Keymap::Main,
        env: PromptEnv {
            in_nix_shell: Some("impure".to_string()),
            nix_shell_name: Some("bench".to_string()),
            home: Some(PathBuf::from("/nonexistent/nova-bench-home")),
            aws: AwsEnv {
                aws_profile: Some("bench".to_string()),
                aws_region: Some("ap-northeast-1".to_string()),
                aws_access_key_id_present: true,
                ..AwsEnv::default()
            },
            ..PromptEnv::default()
        },
    }
}

fn async_values() -> AsyncSegmentValues {
    AsyncSegmentValues::from([
        ("git_branch".to_string(), ready("git_branch", " main")),
        ("git_status".to_string(), ready("git_status", "[!2 +1 ⇡1]")),
        (
            "rust_version".to_string(),
            ready("rust_version", " 1.96.1"),
        ),
        ("bun_version".to_string(), ready("bun_version", " 1.2.18")),
        ("deno_version".to_string(), ready("deno_version", " 2.3.6")),
        ("node_version".to_string(), AsyncValue::Failed),
        (
            "python_version".to_string(),
            ready("python_version", " 3.12.4"),
        ),
    ])
}

fn ready(id: &str, text: &str) -> AsyncValue {
    AsyncValue::Ready(SegmentContent::new(id, text, Style::default()))
}
