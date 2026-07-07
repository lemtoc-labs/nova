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

| metric                 |     green | yellow limit | orange limit |       red |
| ---------------------- | --------: | -----------: | -----------: | --------: |
| `first_prompt_lag_ms`  | `<= 25ms` |    `<= 50ms` |   `<= 100ms` | `> 100ms` |
| `first_command_lag_ms` | `<= 75ms` |   `<= 150ms` |   `<= 300ms` | `> 300ms` |
| `command_lag_ms`       |  `<= 5ms` |    `<= 10ms` |    `<= 20ms` |  `> 20ms` |
| `input_lag_ms`         | `<= 10ms` |    `<= 20ms` |    `<= 40ms` |  `> 40ms` |

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

## Measurement Log

### 2026-07-08: PR-3 Preflight Checkpoint

Environment:

- Git revision: `e1a30e6` (`fix(worker): correct async pipeline behavior`)
- Machine: Apple Silicon `arm64` (local M4 Pro workstation)
- Shell benchmark: `zsh-bench` via `scripts/bench-zsh`
- Binary: `target/release/nova`, built with `nix develop -c cargo build --release`

Commands:

```sh
NOVA_CONFIG=/private/tmp/nova-pr3-bench/sync-only.toml scripts/bench-zsh
scripts/bench-zsh
NOVA_ZSH_BENCH_ITERS=32 scripts/bench-zsh
```

The sync-only config removed `git_*` and `*_version` segments from the default
layout, leaving only synchronous prompt content.

| checkpoint | metric | n | min | median | p95 | max |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| M-1 sync-only | `first_prompt_lag_ms` | 16 | 23.085 | 26.926 | 29.819 | 35.497 |
| M-1 sync-only | `first_command_lag_ms` | 16 | 23.235 | 27.060 | 29.961 | 35.648 |
| M-1 sync-only | `command_lag_ms` | 16 | 0.214 | 0.302 | 0.312 | 0.313 |
| M-1 sync-only | `input_lag_ms` | 16 | 0.129 | 0.572 | 0.693 | 0.769 |
| M-2 default | `first_prompt_lag_ms` | 16 | 25.092 | 27.096 | 30.579 | 38.396 |
| M-2 default | `first_command_lag_ms` | 16 | 25.221 | 27.329 | 30.759 | 38.559 |
| M-2 default | `command_lag_ms` | 16 | 0.339 | 0.396 | 0.411 | 0.417 |
| M-2 default | `input_lag_ms` | 16 | 0.135 | 0.583 | 0.740 | 0.757 |
| M-3 default, 32 iterations | `first_prompt_lag_ms` | 32 | 21.624 | 27.104 | 28.803 | 37.120 |
| M-3 default, 32 iterations | `first_command_lag_ms` | 32 | 21.760 | 27.288 | 28.996 | 37.394 |
| M-3 default, 32 iterations | `command_lag_ms` | 32 | 0.361 | 0.397 | 0.411 | 0.425 |
| M-3 default, 32 iterations | `input_lag_ms` | 32 | 0.117 | 0.534 | 0.649 | 0.672 |

Interpretation:

- M-1 confirms the sync-only baseline has flat input lag, with p95 below 1 ms.
- M-2 shows the default async layout is now similarly flat after the worker
  event-channel and cache correctness fixes.
- M-3 reproduces the PR benchmark iteration count locally and remains stable;
  the default layout's input lag max was 0.672 ms.
- PR-3 can add the `initial_wait_ms` mechanism with its default at `0` without
  changing current behavior. The default value decision remains a separate A/B
  measurement task.

After implementing PR-3 with the default `initial_wait_ms = 0`, the same default
benchmark stayed in the same range:

| checkpoint | metric | n | min | median | p95 | max |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| PR-3 default after | `first_prompt_lag_ms` | 16 | 26.051 | 27.485 | 30.008 | 35.082 |
| PR-3 default after | `first_command_lag_ms` | 16 | 26.315 | 27.744 | 30.264 | 35.309 |
| PR-3 default after | `command_lag_ms` | 16 | 0.371 | 0.400 | 0.424 | 0.424 |
| PR-3 default after | `input_lag_ms` | 16 | 0.229 | 0.538 | 0.690 | 0.713 |
| PR-3 `initial_wait_ms = 10` smoke | `first_prompt_lag_ms` | 4 | 27.159 | 27.284 | 36.794 | 38.453 |
| PR-3 `initial_wait_ms = 10` smoke | `first_command_lag_ms` | 4 | 27.365 | 27.450 | 36.950 | 38.613 |
| PR-3 `initial_wait_ms = 10` smoke | `command_lag_ms` | 4 | 0.398 | 0.411 | 0.415 | 0.415 |
| PR-3 `initial_wait_ms = 10` smoke | `input_lag_ms` | 4 | 0.188 | 0.336 | 0.496 | 0.501 |

### 2026-07-08: PR-4 Initial Wait Default Decision

Environment:

- Git revision: `70647ad` (`feat(worker): add initial render wait budget`)
- Machine: Apple Silicon `arm64` (local M4 Pro workstation)
- Shell benchmark: `zsh-bench` via `scripts/bench-zsh`
- Binary: `target/release/nova`, built with `nix develop -c cargo build --release`

Commands:

```sh
NOVA_CONFIG=/private/tmp/nova-pr4-bench/initial-wait-0.toml scripts/bench-zsh
NOVA_CONFIG=/private/tmp/nova-pr4-bench/initial-wait-10.toml scripts/bench-zsh
NOVA_CONFIG=/private/tmp/nova-pr4-bench/initial-wait-20.toml scripts/bench-zsh
NOVA_CONFIG=/private/tmp/nova-pr4-bench/initial-wait-40.toml scripts/bench-zsh
PATH=/private/tmp/nova-pr4-bench/bin:$PATH NOVA_CONFIG=/private/tmp/nova-pr4-bench/initial-wait-0.toml scripts/bench-zsh
PATH=/private/tmp/nova-pr4-bench/bin:$PATH NOVA_CONFIG=/private/tmp/nova-pr4-bench/initial-wait-10.toml scripts/bench-zsh
PATH=/private/tmp/nova-pr4-bench/bin:$PATH NOVA_CONFIG=/private/tmp/nova-pr4-bench/initial-wait-20.toml scripts/bench-zsh
PATH=/private/tmp/nova-pr4-bench/bin:$PATH NOVA_CONFIG=/private/tmp/nova-pr4-bench/initial-wait-40.toml scripts/bench-zsh
```

The slow-git condition prepended a wrapper that delayed only Nova's
`git --no-optional-locks status ...` collector command by 30 ms.

| condition | wait | metric | n | min | median | p95 | max |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| normal | 0 | `first_prompt_lag_ms` | 16 | 25.915 | 27.259 | 29.734 | 34.978 |
| normal | 0 | `command_lag_ms` | 16 | 0.375 | 0.414 | 0.432 | 0.434 |
| normal | 0 | `input_lag_ms` | 16 | 0.180 | 0.581 | 0.690 | 0.702 |
| normal | 10 | `first_prompt_lag_ms` | 16 | 25.823 | 27.216 | 30.385 | 38.080 |
| normal | 10 | `command_lag_ms` | 16 | 0.336 | 0.384 | 0.428 | 0.434 |
| normal | 10 | `input_lag_ms` | 16 | 0.118 | 0.426 | 0.632 | 0.687 |
| normal | 20 | `first_prompt_lag_ms` | 16 | 39.154 | 39.959 | 48.773 | 50.833 |
| normal | 20 | `command_lag_ms` | 16 | 0.391 | 0.426 | 0.448 | 0.479 |
| normal | 20 | `input_lag_ms` | 16 | 0.404 | 0.617 | 0.682 | 0.704 |
| normal | 40 | `first_prompt_lag_ms` | 16 | 32.811 | 39.521 | 42.196 | 42.647 |
| normal | 40 | `command_lag_ms` | 16 | 0.344 | 0.417 | 0.430 | 0.430 |
| normal | 40 | `input_lag_ms` | 16 | 0.165 | 0.559 | 0.707 | 0.710 |
| slow git | 0 | `first_prompt_lag_ms` | 16 | 26.626 | 27.467 | 29.858 | 35.292 |
| slow git | 0 | `command_lag_ms` | 16 | 0.304 | 0.372 | 0.385 | 0.385 |
| slow git | 0 | `input_lag_ms` | 16 | 0.157 | 0.260 | 0.394 | 0.490 |
| slow git | 10 | `first_prompt_lag_ms` | 16 | 22.616 | 27.359 | 30.417 | 36.163 |
| slow git | 10 | `command_lag_ms` | 16 | 0.344 | 0.379 | 0.393 | 0.394 |
| slow git | 10 | `input_lag_ms` | 16 | 0.202 | 0.250 | 0.451 | 0.550 |
| slow git | 20 | `first_prompt_lag_ms` | 16 | 39.775 | 42.062 | 45.995 | 51.193 |
| slow git | 20 | `command_lag_ms` | 16 | 0.329 | 0.374 | 0.382 | 0.386 |
| slow git | 20 | `input_lag_ms` | 16 | 0.124 | 0.262 | 0.432 | 0.728 |
| slow git | 40 | `first_prompt_lag_ms` | 16 | 58.147 | 67.072 | 70.684 | 75.085 |
| slow git | 40 | `command_lag_ms` | 16 | 0.325 | 0.374 | 0.393 | 0.398 |
| slow git | 40 | `input_lag_ms` | 16 | 0.187 | 0.239 | 0.462 | 0.646 |

Decision:

- Keep the default `initial_wait_ms` at `0`.
- `input_lag_ms` is already far below the 10 ms green threshold at `0` in both
  normal and slow-git conditions.
- `10` ms does not materially improve command or input lag.
- `20` and `40` ms increase first prompt lag, with `40` ms reaching a 67.072 ms
  median under the slow-git condition.

### 2026-07-08: PR-5 Eager Worker Spawn Check

Environment:

- Git revision: Task 7-5 commit (`perf(worker): eager spawn zsh worker`)
- Machine: Apple Silicon `arm64` (local M4 Pro workstation)
- Shell benchmark: `zsh-bench` via `scripts/bench-zsh`
- Binary: `target/release/nova`, built with `nix develop -c cargo build --release`

Commands:

```sh
scripts/bench-zsh
```

| checkpoint | metric | n | min | median | p95 | max |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| PR-5 eager spawn | `first_prompt_lag_ms` | 16 | 25.166 | 28.545 | 37.943 | 37.943 |
| PR-5 eager spawn | `first_command_lag_ms` | 16 | 25.307 | 28.676 | 38.125 | 38.125 |
| PR-5 eager spawn | `command_lag_ms` | 16 | 0.381 | 0.463 | 0.495 | 0.495 |
| PR-5 eager spawn | `input_lag_ms` | 16 | 0.130 | 0.555 | 0.759 | 0.759 |

Interpretation:

- All primary metrics remain green after spawning the worker during interactive
  init.
- Compared with the PR-4 normal `initial_wait_ms = 0` run, this minimal
  `.zshrc` benchmark is stable rather than a clear first-prompt improvement
  (`first_prompt_lag_ms` median 27.259 ms before, 28.545 ms after).
- The change removes worker process creation from the first `precmd`, but
  `zsh-bench` also includes the `nova init zsh` path in startup-adjacent
  latency, so the benefit is not visible in this minimal shell benchmark.

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

The preview script writes the repository dashboard template and the latest
`origin/gh-pages:bench/zsh/data.js` into `.preview/zsh-bench-dashboard`, then
prints the local `file://` URL to open in a browser. The `.preview` directory is
ignored by git.
