# Performance

Nova performance work is measured in two layers:

- Rust render-path microbenchmarks for prompt composition and lowering.
- zsh end-to-end benchmarks for shell startup and interactive prompt latency.

## Rust Render Benchmarks

Run:

```sh
cargo bench --bench render
```

The benchmark currently reports:

- `render_default_sync`: default layout with synchronous segments only.
- `render_default_cached_async`: default layout with ready cached async segment values.
- `render_narrow_cached_async`: cached async values with a narrow terminal width,
  forcing truncation/drop behavior.

Record results with the machine model, date, git revision, and command output.
Do not compare results across machines without noting the hardware and shell
environment.

## zsh Benchmarks

Build the release binary first:

```sh
cargo build --release
```

Run the local Nova zsh benchmark:

```sh
scripts/bench-zsh
```

For a quick smoke check:

```sh
NOVA_ZSH_BENCH_ITERS=2 scripts/bench-zsh
```

The runner creates a temporary `HOME`/`ZDOTDIR`, writes a minimal `.zshenv`
and `.zshrc`, and evaluates `nova init zsh` from `target/release/nova`. This
keeps user shell configuration out of the measurement.

The default benchmark uses:

- `--login no`: measure an interactive non-login zsh.
- `--git empty`: run inside an empty git repository.
- `--standalone`: close the warm helper shell before measuring.
- `--raw`: print all iteration values.

Record the raw output with the machine model, date, git revision, terminal,
and any non-default environment variables. The main fields to watch are
`first_prompt_lag_ms`, `first_command_lag_ms`, `command_lag_ms`, and
`input_lag_ms`.

`exit_time_ms` is not used as a Nova performance target. `zsh-bench` reports it
for completeness, but it is not a good proxy for interactive shell latency.

### zsh-bench Thresholds

Use the `zsh-bench` perception thresholds as the baseline for interpreting
results:

| metric | green | yellow limit | orange limit | red |
| --- | ---: | ---: | ---: | ---: |
| `first_prompt_lag_ms` | `<= 25ms` | `<= 50ms` | `<= 100ms` | `> 100ms` |
| `first_command_lag_ms` | `<= 75ms` | `<= 150ms` | `<= 300ms` | `> 300ms` |
| `command_lag_ms` | `<= 5ms` | `<= 10ms` | `<= 20ms` | `> 20ms` |
| `input_lag_ms` | `<= 10ms` | `<= 20ms` | `<= 40ms` | `> 40ms` |

Nova uses those thresholds in three tiers:

- Acceptance: all four primary fields stay within yellow.
- Target: `command_lag_ms` and `input_lag_ms` stay within green.
- Stretch: all four primary fields stay within green.

The target is stricter for `command_lag_ms` and `input_lag_ms` because Nova is
a prompt component, not the user's whole shell configuration. It should leave
latency budget for the user's terminal, plugins, completion, syntax
highlighting, and autosuggestions.

`first_prompt_lag_ms` and `first_command_lag_ms` may include shell startup and
`nova init zsh` overhead, so yellow is acceptable initially. If either startup
field remains near the yellow limit after M6 tuning, revisit initialization
costs separately from the steady-state prompt path.

## GitHub Actions Benchmarks

Nova runs two zsh-bench workflows with different purposes:

- `Zsh Benchmark (PR)` runs on `macos-latest` for pull requests. Use it as a
  regression smoke test and trend signal. GitHub-hosted runner values are not
  fixed-machine measurements.
- `Zsh Benchmark (Self-hosted)` runs on `workflow_dispatch` and pushes to
  `main` with `runs-on: [self-hosted, nova-bench]`. Use it as the fixed-machine
  reference for M4Pro measurements.

The self-hosted workflow is intentionally not a pull request trigger while the
runner is manually started. This avoids leaving ordinary PRs pending when the
local runner is offline. Start the Nova runner with `./run.sh` when a fixed
benchmark is needed, then trigger the workflow from GitHub Actions.

The self-hosted workflow stores benchmark history in the `gh-pages` branch under
`bench/zsh`. Configure GitHub Pages to publish from the `gh-pages` branch root;
the dashboard is then available at `/nova/bench/zsh/`.

Preview the dashboard locally before changing the published template:

```sh
scripts/preview-zsh-bench-dashboard
```

The preview script copies the repository dashboard template and the latest
`origin/gh-pages:bench/zsh/data.js` into a temporary directory, then prints the
local `file://` URL to open in a browser.
