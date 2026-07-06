# Design

## Architecture Overview

Nova is a Rust library plus a single `nova` binary. Each interactive zsh session
spawns one background worker process. zsh and the worker communicate over a pair
of FIFOs. The worker owns configuration, rendering, the in-memory cache, and
asynchronous data collection. zsh stays thin glue: hooks, state capture, fd
monitoring, prompt assignment, and redraws.

Guiding constraints:

- The synchronous render path is the only latency-critical code. Asynchronous
  collection is allowed to be slow.
- No global daemon. No per-prompt renderer in the default interactive path
  (kept only as a bootstrap/debug mode).
- No async runtime. The worker uses `std::thread` plus channels.
- Keep dependencies minimal so the binary stays small and compiles fast.

## Decisions

Previously open questions are now decided. Rationale appears in the linked
sections.

| Topic                        | Decision                                                                                        | Section                 |
| ---------------------------- | ----------------------------------------------------------------------------------------------- | ----------------------- |
| CLI binary                   | Yes. One binary: `nova init zsh`, `nova worker`, `nova prompt`, `nova check`                    | Binary and Crate Layout |
| zsh ↔ worker transport       | FIFO pair + disowned background process, **not** `coproc`; zsh side opens nonblocking + cloexec | Transport               |
| Wire protocol                | Versioned, control-character framed records, **not** JSON                                       | Protocol                |
| Async runtime                | None. `std::thread` + `std::sync::mpsc`                                                         | Slow Path               |
| Git backend                  | `git` subprocess in the async path, no libgit2/gix                                              | Slow Path               |
| Worker startup               | Eager spawn during `nova init zsh`; FIFO fds opened lazily on first `precmd`                    | Process Model           |
| Initial render wait          | zsh waits ≤ 50 ms via `zselect`, then falls back; result applied later via `zle -F`             | zsh Adapter             |
| Disk cache                   | Deferred until measurements justify it                                                          | Cache                   |
| Minimum zsh                  | 5.8                                                                                             | zsh Adapter             |
| Default segments             | `dir`, `git_branch`, `git_status`, `rust_version`, `duration`, `prompt_char`                    | Configuration           |
| Worker sharing across shells | No                                                                                              | Process Model           |
| Rust toolchain               | Latest stable, edition 2024, `#![forbid(unsafe_code)]`                                          | Testing Strategy        |

## High-Level Flow

```text
zsh session
  |
  | eval "$(nova init zsh)"          # emits adapter script, spawns worker (&!)
  v
zsh adapter (shell/init.zsh)
  |
  | two FIFOs in ${XDG_RUNTIME_DIR:-$TMPDIR}/nova-<pid>/
  v
nova worker process
  |
  | event loop: requests + job results + config reload
  v
bounded thread pool -> async collectors (git, runtimes, ...)
```

Per prompt:

```text
precmd
  -> zsh captures exit status, duration, cwd, COLUMNS, keymap
  -> zsh writes a render request to the request FIFO
  -> worker renders fast segments + cached slow segments
  -> worker replies with fully lowered PROMPT/RPROMPT strings
  -> zsh waits up to 50 ms (zselect); applies result or a fallback prompt
  -> worker schedules refresh jobs for stale slow segments
  -> when a job finishes and the composed prompt changed, worker sends Update
  -> zle -F handler reads Update, applies it, runs `zle reset-prompt`
```

## Process Model

Nova must not require a global daemon installed through launchd, systemd, or
another service manager.

Each interactive shell owns one worker:

- `nova init zsh` output spawns `nova worker --dir <runtime-dir>` with `&!`
  (disowned, no job-control noise). Spawning is fire-and-forget and costs a
  fork/exec (~1–2 ms); shell startup never waits for worker readiness.
- The runtime dir is `${XDG_RUNTIME_DIR:-$TMPDIR}/nova-<shell-pid>/` and holds
  `req` and `resp` FIFOs. Before creating it, init removes any pre-existing
  dir for the same shell pid — a leftover from a crashed predecessor or an
  `exec zsh` replacement (the pid survives `exec`).
- Lifetime is tied to the shell: the worker exits when the request FIFO reaches
  EOF (all writers closed — this happens automatically when the shell dies).
  A `zshexit` hook removes the runtime dir as a backup; the worker also removes
  it on clean exit. EOF is only a reliable signal because of the fd-hygiene
  rules below — a leaked write fd would keep the worker alive forever.
- **fd hygiene**: the zsh side opens every Nova fd with `cloexec`, so ordinary
  child commands and `exec zsh` replacements never inherit them. On the Rust
  side, `std` opens fds with `CLOEXEC` by default, so collector children
  (`git`, version probes) never inherit the FIFO fds either.
- **Liveness backstop**: the worker is started with `--shell-pid <pid>`. Its
  event loop uses `recv_timeout` as an idle tick (60 s) and checks
  `kill(pid, 0)` via `libc`; if the shell is gone, the worker exits and cleans
  up. EOF remains the primary exit signal; this catches leaked-writer and
  reparenting edge cases.
- **Session token**: init generates a random token per shell and passes it to
  the worker; the worker echoes it in the handshake. A mismatch means zsh is
  talking to a stale worker from a previous session and is treated like a
  version mismatch (permanent fallback for the session).
- Crash recovery: if a write to the worker fails, zsh marks it dead, shows the
  fallback prompt, and respawns on the next `precmd`. After 3 consecutive
  failed respawns, zsh stops retrying for the session and warns once.
  Nonblocking-open failures (see Transport) count toward the same budget, so
  no failure mode can loop forever.
- Workers are never shared between shells. Cross-shell reuse is a disk-cache
  concern, which is deferred.

## Why Not Per-Prompt Processes

A per-prompt `nova prompt` subprocess is simpler, but conflicts with the
performance and async requirements:

- fork/exec cost on every prompt.
- No in-memory cache, no inflight coalescing.
- No channel for pushing async updates back into zsh.

Nova still ships `nova prompt` as a one-shot command, for three reasons: it is
the milestone M2 bootstrap integration, a debugging tool, and a test harness
for the render pipeline. It shares the exact same rendering code as the worker.

## Why Not A Global Daemon

A global daemon has real advantages (shared cache, no per-shell startup), but
the operational weight is wrong for Nova's initial UX: service installation,
socket lifecycle, version negotiation with running daemons, upgrade edge
cases, and surprising background processes. The per-shell worker keeps most of
the useful statefulness while making lifecycle ownership obvious.

## Transport

**Decision: two FIFOs + a disowned background process. Not `coproc`.**

zsh supports exactly **one** coprocess per shell. If Nova claims the coproc
slot, any user script or plugin that runs `coproc` silently disconnects the
prompt worker. FIFOs have no such conflict, and both directions remain plain
file descriptors that `zle -F` can watch. Only stock zsh modules are needed
(`zsh/system` for `sysopen`/`sysread`, `zsh/zselect` for timed waits,
`zsh/datetime` for durations).

Rails and pitfalls:

- The worker creates nothing; the init script creates the runtime dir and both
  FIFOs with `mkfifo` before spawning the worker, so there is no startup race
  on file existence.
- **Opening a FIFO can block until the other end is opened, and the
  interactive shell must never perform a blocking open.** The worker — where
  blocking is harmless — opens `req` for read, then `resp` for write, with
  plain blocking opens at startup. The zsh side opens its fds **lazily on the
  first `precmd`** (not during init) and **always nonblocking**, via
  `sysopen -o nonblock -o cloexec`:
  - `req` for write first. A nonblocking write-open of a FIFO with no reader
    fails with `ENXIO`; zsh treats that as "worker not ready or dead", retries
    once after a 10 ms `zselect` sleep, then falls back and counts one failure
    toward the respawn budget. It never waits longer.
  - `resp` for read second. A nonblocking read-open succeeds regardless of the
    worker's state, and the worker's pending `resp` write-open completes at
    that moment.
    This ordering has no interleaving that deadlocks and no path where zsh
    blocks indefinitely — a dead, hung, or missing worker always resolves to
    the fallback prompt within the retry budget.
- One writer per FIFO in each direction. The worker writes `resp` only from its
  main thread, so records are never interleaved.

Rejected transports:

- `coproc`: single-slot conflict described above.
- Unix domain socket: needs `zsh/net/socket`, more zsh-side code, and solves no
  problem FIFOs have here.
- `zpty`: allocates a pseudo-terminal per shell and has fragile edge cases.

## Protocol

**Decision: versioned, control-character framed records. Not JSON.**

zsh cannot parse JSON without external tools, and prompt payloads contain
newlines and arbitrary styled text. The frame format is chosen so that zsh can
split it with nothing but parameter expansion:

- Field separator: `NUL` (`\x00`). Paths cannot contain NUL, and zsh variables
  can hold and split on NUL (`${(0)...}` / `${(ps:\0:)...}`), unlike bash.
- Record terminator: `RS` (`\x1e`). The worker guarantees it never emits NUL or
  RS inside payload fields (it strips control characters from all segment
  text). A cwd containing RS is documented as unsupported.

Records (version 1):

```text
worker -> zsh, once at startup (handshake):
  H <NUL> 1 <NUL> session_token <RS>

zsh -> worker, one per precmd:
  R <NUL> gen <NUL> cwd <NUL> exit_status <NUL> duration_ms <NUL> cols <NUL> keymap <RS>

worker -> zsh, first reply for a generation:
  P <NUL> gen <NUL> final|partial <NUL> PROMPT <NUL> RPROMPT <RS>

worker -> zsh, async update (same shape, same gen, later record wins):
  U <NUL> gen <NUL> final|partial <NUL> PROMPT <NUL> RPROMPT <RS>
```

Rules:

- `gen` is a monotonically increasing prompt generation chosen by zsh. The zsh
  side stores the last applied `gen` and discards any record with a smaller
  one; the worker drops queued work for superseded generations.
- `partial` means async data is still loading or stale; `final` means every
  active async value is ready or failed. zsh does not interpret this beyond
  debugging. The worker renders `Ready` and `Stale` values into the prompt and
  omits `Loading`/`Failed` values so unavailable async data does not move the
  input line. `Failed` is settled unavailable, so it does not force a
  status-only update when prompt text is unchanged.
- On a version mismatch in the handshake (stale binary vs. new init script),
  zsh permanently falls back to the plain prompt for the session and warns
  once. A `session_token` mismatch (stale worker from a previous shell) is
  treated the same way.
- **All protocol I/O on the zsh side must use `zsh/system` `sysread` and
  `syswrite`.** The `read` builtin cannot deliver NUL bytes intact and must
  never touch protocol data; `sysread` also gives correct partial-read
  semantics for the buffering rule below.
- `PROMPT`/`RPROMPT` are fully lowered, zsh-escaped strings. zsh assigns them
  verbatim; it never edits prompt content.
- Torn reads are normal: the `zle -F` handler appends `sysread` output to a
  buffer and processes only complete `RS`-terminated records.

`src/worker/protocol.rs` owns encode/decode for both directions and is
round-trip tested, including torn-frame cases.

## Binary and Crate Layout

Single crate, library plus one binary. A workspace is not justified yet.

```text
nova/
├── Cargo.toml
├── src/
│   ├── lib.rs             # public API, re-exports, #![forbid(unsafe_code)]
│   ├── config/
│   │   ├── mod.rs         # Config, LayoutConfig, SegmentConfig (serde structs)
│   │   ├── load.rs        # discovery + mtime-based reload + generation bump
│   │   └── error.rs       # human-readable validation errors
│   ├── state.rs           # PromptState: per-prompt inputs from zsh
│   ├── segments/
│   │   ├── mod.rs         # SegmentId, registry, SyncSegment/AsyncSegment traits
│   │   ├── dir.rs         # sync
│   │   ├── prompt_char.rs # sync (exit status + keymap aware)
│   │   ├── duration.rs    # sync
│   │   ├── ssh.rs         # sync
│   │   ├── git.rs         # async: branch + status via git subprocess
│   │   └── runtime.rs     # async: node/rust/python/... via markers then commands
│   ├── render/
│   │   ├── mod.rs         # compose RenderedPrompt from state + config + cache
│   │   ├── width.rs       # display width (unicode-width), truncation
│   │   └── zsh.rs         # lowering: %-escaping, %{...%} wrapping, line1 padding
│   ├── cache.rs           # bounded freshness cache + inflight set
│   ├── worker/
│   │   ├── mod.rs         # event loop (single mpsc receiver, enum Event)
│   │   ├── protocol.rs    # frame encode/decode, shared with tests
│   │   └── jobs.rs        # bounded thread pool, deadlines, supersession
│   ├── zsh_init.rs        # embeds shell/init.zsh via include_str!
│   └── main.rs            # subcommands: init | worker | prompt | check
└── shell/
    └── init.zsh           # the zsh adapter
```

Layering rules (enforced by review, testable by imports):

- `render`, `segments`, `config`, `state`, `cache` know nothing about FIFOs,
  processes, or zsh lifecycles. They are pure enough to unit-test directly.
- `worker` knows the protocol and the pipeline but contains no rendering rules.
- `shell/init.zsh` contains no rendering, git, or cache logic.

Dependency policy — allowed: `serde`, `toml`, `unicode-width`, `thiserror`,
`wait-timeout` (tiny; child-process timeouts), `libc` (only for the
`kill(pid, 0)` shell-liveness check). Dev-only: `insta`, `proptest`,
`assert_cmd`. Not allowed without a measured justification: `tokio`/any async
runtime, `clap` (subcommand dispatch is a hand-rolled `match`), `git2`/`gix`,
`chrono`.

## Core Types

Sketches, not final signatures. They pin the shape a junior should implement.

```rust
/// Everything zsh sends per prompt. No I/O happens to build this.
pub struct PromptState {
    pub cwd: PathBuf,
    pub exit_status: i32,
    pub duration_ms: Option<u64>,
    pub columns: u16,
    pub keymap: Keymap, // Main | Vicmd
}

pub struct SegmentContent {
    pub text: String, // control characters already stripped
    pub style: Style, // fg/bg/bold; lowered to ANSI only in render::zsh
}

/// State of an async segment as seen by the renderer.
pub enum AsyncValue {
    Loading,
    Ready(SegmentContent),
    Stale(SegmentContent),
    Failed,
}

pub trait SyncSegment {
    fn id(&self) -> &'static str;
    /// Must be cheap and deterministic: no subprocesses, no network,
    /// at most a couple of env reads / stat calls.
    fn render(&self, state: &PromptState, cfg: &SegmentConfig) -> Option<SegmentContent>;
}

pub trait AsyncSegment {
    fn id(&self) -> &'static str;
    /// None = segment not applicable here (e.g. git outside a repo candidate).
    fn cache_key(&self, state: &PromptState) -> Option<CacheKey>;
    /// Runs on a pool thread. May block, must respect the deadline.
    fn collect(&self, state: &PromptState, deadline: Instant)
        -> Result<SegmentContent, CollectError>;
}

/// Structured output; the zsh adapter never sees this, only its lowering.
pub struct RenderedPrompt {
    pub line1_left: Vec<SegmentContent>,
    pub line1_right: Vec<SegmentContent>, // composed via padding, see Rendering
    pub line2_left: Vec<SegmentContent>,  // empty in one-line layout
    pub line2_right: Vec<SegmentContent>, // becomes RPROMPT
}
```

## Rendering Rules

The worker lowers `RenderedPrompt` into final `PROMPT`/`RPROMPT` strings; zsh
only assigns them.

- **First-line right prompt**: zsh's `RPROMPT` renders only on the prompt's
  last line. The right block of line 1 in a two-line layout therefore must be
  composed in Rust: `line1 = left + spaces(cols − width(left) − width(right)) + right`.
  This is the single most error-prone rendering feature; it gets dedicated
  snapshot and property tests.
- **Width**: computed in Rust from unstyled text using `unicode-width`.
  Nerd Font private-use glyphs are treated as width 1 by default, with a
  per-icon override in config for terminals that disagree.
- **Escaping**: literal `%` in any dynamic text becomes `%%`. ANSI SGR
  sequences are wrapped in `%{...%}` so zsh excludes them from its own width
  accounting. Nova never relies on zsh `%F{...}` style codes for dynamic
  content, so there is exactly one styling path to test.
- **Truncation priority** when `cols` is insufficient: (1) shorten the dir
  path keeping the last components, (2) truncate the branch name with an
  ellipsis, (3) drop `line1_right`, (4) drop `line2_right`.
- **Coalescing**: the worker remembers the last strings sent for the current
  generation and sends an Update only when the composed output differs.
  Redraw coalescing therefore requires no zsh-side logic.

## Fast Path

Runs before the prompt is displayed. Budget: see requirements (< 2 ms render).

Allowed inputs: cwd, exit status, duration captured by zsh, prompt character,
keymap, terminal width, cheap env vars already sent by zsh, and cached slow
segment output already in memory.

Disallowed: `git` commands, tool version commands, network, filesystem walks,
and any disk read that can miss the budget.

The fast path is benchmarked in isolation (Criterion or `cargo bench` harness)
and end-to-end via `zsh-bench`.

## Slow Path

Runs on the worker's thread pool, never on the prompt path. **Slow-path speed
is explicitly a non-goal**; simplicity and isolation win every tradeoff here.

- **No async runtime.** A bounded pool (default 2 threads) of `std::thread`
  workers consuming a job queue via `std::sync::mpsc`. This collapses the
  learning curve, compile time, and binary size compared to tokio, and prompt
  latency does not depend on it.
- Every job gets a deadline (default `timeout_ms = 1000`, per-segment
  override). Child processes are waited on with `wait-timeout` and killed on
  expiry; the cache records `Failed` when there is no previous value, or keeps
  the previous success as `Stale`.
- Supersession: jobs carry the generation that scheduled them. Completed
  results are always stored in the cache (still useful later), but an Update
  is pushed only if the _current_ prompt's composition changes.
- Failures are isolated per segment: a panicking collector thread is caught
  (`catch_unwind` around the job body) and recorded as `Failed` without
  affecting other segments.

Git collection (rail):

- One subprocess per refresh:
  `git --no-optional-locks status --porcelain=v2 --branch --show-stash -z`
  yields branch, upstream, ahead/behind, staged/unstaged/untracked/conflicts,
  and the stash count (`# stash <N>` header) in a single parse. On git older
  than 2.35 the stash header is absent; treat that as 0.
- **Nova never reads files under `.git` directly.** Repository internals vary
  too much across layouts — in worktrees and submodules `.git` is a file
  containing `gitdir: ...`, and shared refs/logs live under the common git
  dir. Letting the `git` binary resolve all of that is exactly why the
  subprocess backend was chosen.
- Repository detection (for `cache_key`) walks up from cwd looking for
  `.git`, accepting **either a directory or a file** as the repo marker;
  this is a bounded stat-walk done on the pool thread, not the prompt path.

## Cache

Owned by the worker. One layer for now.

- One entry per `(segment_id, CacheKey)`. `CacheKey` includes the repo root or
  cwd, the config generation, and segment-relevant markers (lockfile paths,
  profile names).
- Value carries `collected_at` and the config generation.
- Bounded at 128 entries. Eviction: drop the oldest `collected_at` when full.
  (A true LRU is an optional upgrade; do not start there.)
- **Stale-while-revalidate**: an expired entry still renders as `Stale` while
  a refresh job runs.
- **Inflight set**: a `HashSet<CacheKey>` prevents duplicate concurrent jobs
  for the same key.
- TTLs are per segment: `git_status` defaults to `ttl_ms = 0` (serve cached
  immediately, always revalidate in the background after each prompt — the
  user may have just committed), runtime versions default to `ttl_ms =
300_000`.
- Invalidation: config reload bumps the generation, which changes every key.

Disk cache stays deferred. If measurements later justify it (cold-start UX),
it must be bounded, atomically written, corruption-tolerant, and never on the
blocking path — and it must not replace the in-memory inflight set.

## Worker Event Loop

Single-threaded main loop; the classic std pattern a junior can follow:

```rust
enum Event {
    Request(RenderRequest),   // decoded frames, forwarded by the reader thread
    JobDone(JobResult),       // sent by pool threads
    ReaderClosed,             // req FIFO hit EOF -> shell died -> exit
}
```

- A dedicated reader thread blocks on the `req` FIFO, decodes frames, and
  forwards `Event::Request` into one `mpsc::Sender<Event>` shared with the
  pool.
- The main loop is the only writer to the `resp` FIFO and the only owner of
  the cache — no locks anywhere.
- On each request: `stat` the config file (mtime + size) and reload on change;
  render; reply; schedule refresh jobs for stale/missing async segments.

## zsh Adapter

Responsibilities (and nothing more): register `precmd`/`preexec`; capture `$?`
first; measure duration via `zsh/datetime` `EPOCHREALTIME` deltas; capture
`PWD`, `COLUMNS`, keymap; spawn/respawn the worker; lazily open FIFO fds
(always `sysopen -o nonblock -o cloexec`, never a blocking open); do protocol
I/O only with `sysread`/`syswrite`; send
requests; wait for the first reply with `zselect` (≤ 50 ms), else apply the
fallback prompt `%~ %# `; watch the resp fd with `zle -F`; apply
`PROMPT`/`RPROMPT`; call `zle reset-prompt` only when zle is active and the
strings actually changed.

Skeleton (rail, not final code):

```zsh
# emitted by `nova init zsh`
zmodload zsh/system zsh/zselect zsh/datetime

_nova_preexec() { _nova_cmd_start=$EPOCHREALTIME }
_nova_precmd() {
  local exit=$?
  _nova_ensure_worker || { _nova_fallback; return }
  (( _nova_gen++ ))
  _nova_send R $_nova_gen $PWD $exit $_nova_duration_ms $COLUMNS $KEYMAP \
    || { _nova_mark_dead; _nova_fallback; return }
  zselect -t 5 -r $_nova_resp_fd && _nova_drain || _nova_fallback  # 5 = 50ms
}
_nova_on_update() { _nova_drain && zle && zle reset-prompt }
zle -F $_nova_resp_fd _nova_on_update
```

Minimum supported zsh is 5.8 (ships with macOS; all modules used are stock).

## Configuration

TOML, discovered as `$NOVA_CONFIG` → `$XDG_CONFIG_HOME/nova/nova.toml` →
built-in defaults. Missing file is not an error.

```toml
[layout]
lines = 2                       # 1 or 2

[layout.line1]
left  = ["dir", "git_branch", "git_status", "rust_version"]
right = ["duration"]

[layout.line2]
left  = ["prompt_char"]

[async]
max_concurrency = 2
timeout_ms = 1000               # per-segment override: [segments.<id>] timeout_ms
ttl_ms = 300000                 # per-segment override: [segments.<id>] ttl_ms

[segments.dir]
max_components = 4
style = { fg = "blue", bold = true }

[segments.git_status]
ttl_ms = 0
icons = { staged = "", conflicted = "", stash = "" }

[segments.rust_version]
icon = ""                     # set to "" to hide the icon
ttl_ms = 300000
timeout_ms = 1000

[segments.duration]
min_ms = 2000                   # hide below this
```

Rules:

- Parsing/validation lives in `config`, reported with file/key context.
  `nova check` prints the same diagnostics and exits non-zero.
- An invalid config **never breaks the shell**: the worker falls back to
  built-in defaults and emits a single-line stderr warning once.
- Reload: mtime+size check per render request; on change, reparse and bump the
  config generation (invalidating the cache). No file watcher needed.
- Every segment has a stable snake_case id; unknown ids in `layout` are a
  validation warning, not a crash.

## Testing Strategy

Test pyramid, from cheap to expensive:

1. **Unit tests** colocated in each module (`config`, `width`, `cache`,
   segment parsers — especially the porcelain-v2 parser with fixture inputs).
2. **Snapshot tests** (`insta`) for the renderer: every layout combination
   (1-line, 2-line, with/without rprompt, first-line right), loading/stale/
   failed state behavior, truncation steps, `%`-escaping. Snapshots review the
   exact lowered `PROMPT`/`RPROMPT` strings.
3. **Property tests** (`proptest`) for width/truncation invariants: rendered
   visible width never exceeds `cols`; no panics for arbitrary Unicode paths
   and branch names; escaping round-trips.
4. **Protocol tests**: encode/decode round-trips, torn frames split at every
   byte boundary, garbage tolerance.
5. **Worker integration tests** (`assert_cmd` + std fs): spawn the real
   `nova worker` against FIFOs created by the test, drive request/response,
   assert generation handling and update sequencing. Must include timeout
   correctness: a collector whose child (e.g. `sleep`) outlives the deadline
   is killed, reports `Failed`/stale, and leaves no zombie. **No zsh
   required.**
6. **zsh e2e smoke** (optional, CI-gated on zsh availability): a `zpty`-driven
   script asserting a prompt appears and updates.

Quality gates (CI, in order): `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test`,
`cargo llvm-cov` with ≥ 80 % line coverage over `config`, `render`,
`segments`, `cache`, and `worker::protocol` (the event loop and `main.rs` are
covered by the integration tier instead). The library sets
`#![forbid(unsafe_code)]`.

## Implementation Milestones

Each milestone is shippable and has explicit acceptance criteria. Do them in
order; do not start async work before the sync renderer is snapshot-tested.

- **M0 — Scaffolding.** Crate layout above, CI with all quality gates, empty
  module stubs. _Accept: CI green on the empty skeleton._
- **M1 — Core rendering.** Config model, `PromptState`, sync segments (`dir`,
  `prompt_char`, `duration`, `ssh`), renderer with all four layout quadrants,
  truncation, lowering/escaping, `nova prompt` one-shot command. _Accept:
  snapshot + property tests pass; `nova prompt --cwd ~/x --cols 80 --exit 1`
  prints a correct two-line prompt._
- **M2 — Bootstrap zsh integration.** Init-script variant that calls
  `nova prompt` per precmd. No worker, no async. _Accept: usable prompt in a
  real zsh; `zsh-bench` baseline recorded in the repo._
- **M3 — Worker mode.** FIFOs, protocol, event loop, handshake, fallback and
  respawn. Fast segments only. _Accept: protocol + worker integration tests
  pass; `kill -9` of the worker leaves a working (fallback) prompt that
  recovers on the next prompt._
- **M4 — Async pipeline.** Thread pool, cache, inflight coalescing, git
  segment, `zle -F` updates. _Accept: entering a cold repo shows the prompt
  instantly and git status appears shortly after without moving the input
  line; superseded generations are never applied._
- **M5 — Runtime segments.** `runtime.rs` (marker files first, versions via
  commands), SSH context, timeouts, stale UX polish.
- **M6 — Performance.** `zsh-bench` against the budgets in requirements,
  fast-path microbenchmarks, tuning, measured results documented.

## Measurement Requirements

Nova measures at least: `zsh-bench` startup overhead; time from precmd request
to first reply; time from reply to first Update; cache hit rate; inflight
coalescing count; worker startup time; redraw count per prompt generation.

If the per-shell worker cannot meet the startup and render budgets, revisit
this design.

## Rejected Alternatives

- **Global daemon as default** — lifecycle management too heavy (see above).
- **Per-prompt renderer as default** — defeats caching and async updates;
  retained only as the M2 bootstrap and a debug mode.
- **`coproc` transport** — zsh has a single coproc slot; any other plugin
  using `coproc` silently breaks the prompt.
- **Unix socket transport** — extra module and code for no benefit over FIFOs.
- **`zpty` transport** — per-shell pseudo-terminals, fragile.
- **JSON protocol** — zsh cannot parse JSON natively; payloads contain
  newlines.
- **tokio (any async runtime)** — the pool needs 2 threads and timeouts;
  an async runtime buys nothing here and costs compile time, binary size, and
  a steeper learning curve.
- **libgit2 / gix** — heavy dependency for the async path, whose speed is a
  non-goal; the `git` CLI matches user-observed behavior exactly.

## Open Questions

- Disk cache format and location, if measurements ever justify adding it.
- When to ship a non-Nerd-Font preset (deferred past the first release).
- Whether ambiguous-width Nerd Font glyphs need per-terminal width overrides
  beyond the per-icon config knob.
