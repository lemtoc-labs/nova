# Requirements

## Purpose

Nova is a zsh prompt system implemented as a Rust library.

The project provides a fast, customizable prompt renderer for zsh. It must support rich prompt layouts and asynchronous status collection while keeping the synchronous prompt path small enough to benchmark and reason about.

## Background

Shell prompts easily become slow when they synchronously inspect Git repositories, language runtimes, package managers, cloud CLIs, or Kubernetes state. Nova treats those values as asynchronous data sources. The first render must stay responsive, and expensive information should arrive through cache updates and prompt redraws.

## Terminology

- **Synchronous render path**: Work performed before zsh can display the prompt.
- **Asynchronous render path**: Work performed outside the prompt's blocking path, followed by a prompt redraw when new data is available.
- **Segment**: A small unit of prompt content, such as current directory, Git branch, command duration, or Node.js version.
- **Layout**: A prompt arrangement such as one-line, two-line, left prompt, or right prompt.
- **Portable**: Able to work across different machines, operating systems, terminals, fonts, and installed tools. Nova values portability, but speed and customization are higher priorities. Nerd Font support is an explicit requirement, so a non-icon fallback may be supported but is not the primary experience.

## Goals

- Provide a Rust library as the core product.
- Support zsh as the primary shell integration target.
- Support one-line and two-line prompt layouts.
- Support right-side prompts.
- Support a right-side prompt on the first line of a two-line prompt.
- Use Nerd Font icons in the default visual language.
- Optimize for speed, measured with `zsh-bench` and focused prompt-path benchmarks.
- Optimize for customization of layout, segments, symbols, colors, and async behavior.
- Render Git status and tool/runtime information asynchronously.
- Use a per-shell worker process instead of a global daemon for the default interactive architecture.

## Non-Goals

- Supporting shells other than zsh is not required for the initial version.
- A global daemon managed by launchd, systemd, or another service manager is not required for the initial version.
- Matching an existing prompt theme such as Starship or Powerlevel10k exactly is not required.
- Synchronous Git status inspection is not allowed, even as a convenience shortcut.
- Tool/runtime version probing must not block the initial prompt render.
- A fully portable no-icon theme is not required for the initial version.

## Functional Requirements

### Rust Library

- The core implementation must be a Rust library crate.
- The library must expose prompt rendering primitives that can be used by a zsh adapter.
- The library must keep rendering, configuration, data collection, and zsh integration separable.
- The public API should avoid requiring callers to spawn shell commands directly.
- The public API should represent prompt output in a structured way, including left prompt, right prompt, line breaks, styles, and width-sensitive metadata.

### zsh Integration

- The zsh integration must support `PROMPT` / `PS1` rendering.
- The zsh integration must support `RPROMPT` / `RPS1` rendering.
- The zsh integration must support one-line prompts.
- The zsh integration must support two-line prompts.
- The zsh integration must support a right-side prompt on the first line of a two-line prompt.
- The integration must account for zsh prompt escaping so non-printing sequences do not break cursor positioning.
- The integration must redraw the prompt when asynchronous data changes.
- The integration must avoid visible flicker during async updates.
- The integration must degrade gracefully when async data is unavailable, stale, or still loading.

### Prompt Layout

- Users must be able to choose between one-line and two-line layouts.
- Users must be able to configure which segments appear on each side and line.
- Users must be able to configure separators, icons, colors, and empty-state behavior.
- The renderer must handle terminal width changes.
- The renderer must handle truncation for long paths, long branch names, and long right prompts.
- The renderer must preserve a stable input area so async updates do not shift the command being typed.

### Segments

The initial segment model should support:

- Current working directory.
- Git branch or detached HEAD state.
- Git repository status.
- Last command exit status.
- Command execution duration.
- SSH context.
- Runtime and tool information, such as Rust, Node.js, Python, package manager, cloud profile, or Kubernetes context.

Segment requirements:

- Segments must have stable identifiers for configuration.
- Segments must define whether they are synchronous or asynchronous.
- Synchronous segments must be cheap and deterministic.
- Git status must be asynchronous.
- Tool/runtime information must be asynchronous unless it is already available from a cheap cache.
- Async segments must support loading, success, stale, error, and disabled states.

### Git Status

- Git branch detection must not block the synchronous prompt path.
- Dirty status, staged status, untracked status, ahead/behind counts, stash count, and conflict state must be collected asynchronously.
- The prompt must support showing cached Git data immediately.
- Git data must be invalidated on directory changes and relevant repository changes.
- Git data collection must be cancellable or superseded when the user changes directories quickly.
- Git data collection must avoid spawning unnecessary Git commands.

### Tool and Runtime Information

- Tool/runtime version detection must be asynchronous.
- Tool/runtime detection must be opt-in or cheaply auto-detected per segment.
- Detection should prefer project markers such as lockfiles, manifest files, or version files before executing tool commands.
- Missing tools must not produce prompt errors.
- Slow tools must be timeout-bound and report stale or unavailable data instead of blocking.

### Customization

- Users must be able to configure Nova without recompiling.
- Configuration should support a human-editable format, preferably TOML.
- Users must be able to define layout presets.
- Users must be able to enable, disable, reorder, and restyle segments.
- Users must be able to customize Nerd Font icons.
- Users must be able to configure async refresh intervals, cache behavior, and timeouts.
- Configuration errors must be reported clearly without breaking the shell session.

### Performance

- Nova must be benchmarked with `zsh-bench`.
- Nova must provide focused benchmarks for the Rust synchronous render path.
- The synchronous render path must not run Git status commands.
- The synchronous render path must not run tool/runtime version commands.
- Async workers must have bounded concurrency.
- Async workers must have timeouts.
- Prompt redraws must be coalesced so rapid async updates do not repeatedly redraw the prompt.
- Performance budgets should be explicit before implementation begins and updated with measured results.

Initial target budgets:

- Add no more than 10 ms to interactive zsh startup in a minimal configuration.
- Render cached prompt output in under 2 ms on a typical developer machine.
- Keep async Git status collection outside the blocking prompt path.
- Keep slow external tool probes from blocking prompt display entirely.

### Quality and Testing

- The implementation must use the latest stable Rust and edition 2024.
- The library must set `#![forbid(unsafe_code)]`.
- CI must enforce `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`.
- The renderer must be covered by snapshot tests for every supported layout, including truncation and loading/stale/error placeholder states.
- Width and truncation invariants must be covered by property-based tests (arbitrary Unicode paths and branch names must never overflow the terminal width or panic).
- The zsh-to-worker protocol must be covered by round-trip tests, including torn/partial frames.
- The worker must be integration-tested by driving its transport directly, without launching zsh.
- Line coverage must be measured with `cargo llvm-cov`; core modules (config, render, segments, cache, protocol) must stay at or above 80%.

### Reliability

- Nova must not prevent the user from typing commands if the Rust component fails.
- Nova must provide a plain fallback prompt when rendering fails.
- Async failures must be isolated to the affected segment.
- The prompt must remain usable inside non-Git directories.
- The prompt must remain usable when external commands are missing.
- The prompt must handle Unicode width correctly for icons and multibyte paths.

## Architecture Requirements

The implementation should be organized around these responsibilities:

- **Core renderer**: Converts prompt state and configuration into structured prompt output.
- **Configuration model**: Parses and validates user configuration.
- **Segment registry**: Defines available segments and their sync or async behavior.
- **Async collector**: Runs expensive data collection outside the synchronous prompt path.
- **Cache**: Stores recent async segment values with freshness metadata.
- **zsh adapter**: Connects zsh hooks, prompt variables, redraw behavior, and shell escaping to the Rust library.

The Rust library should make it possible to test rendering without launching zsh.

The default interactive architecture is described in [design.md](design.md). Nova should use a per-shell worker process spawned by zsh and connected over FIFOs as the primary stateful component, not a global daemon and not a zsh coprocess (zsh has a single coprocess slot, which other plugins may claim). The worker owns the in-memory slow segment cache, inflight coalescing, and asynchronous segment refreshes.

## Acceptance Criteria

- `docs/requirements.md` captures the initial product requirements.
- Requirements explicitly state that Nova is a Rust library for zsh prompts.
- Requirements cover one-line prompts, two-line prompts, right prompts, and first-line right prompts for two-line layouts.
- Requirements state that Nerd Font icons are part of the expected experience.
- Requirements prioritize speed and customization.
- Requirements state that Git status and tool/runtime information must be asynchronously rendered.
- Requirements include `zsh-bench` as a required benchmark tool.
- Requirements explain the meaning and priority of portability.

## Resolved Questions

The following questions are resolved; rationale lives in the Decisions section of [design.md](design.md):

- Nova ships a single `nova` CLI binary (`init`, `worker`, `prompt`, `check`) alongside the library.
- Default segments: `dir`, `git_branch`, `git_status`, `duration`, `prompt_char`.
- Minimum supported zsh version: 5.8.
- The non-Nerd-Font fallback theme is deferred past the first release; icons remain user-overridable in config.
- No async runtime: the worker uses `std::thread` with a bounded pool, because asynchronous path speed is a non-goal.
- Worker startup is eager (spawned during `nova init zsh`), but zsh opens the transport lazily on the first prompt so shell startup never blocks.

## Open Questions

- What measured `zsh-bench` budget is acceptable on the user's machine? (To be answered by the M2 baseline measurement.)
