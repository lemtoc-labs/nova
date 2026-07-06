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

End-to-end `zsh-bench` measurements are still pending. They should cover:

- Interactive zsh startup overhead after `eval "$(nova init zsh)"`.
- Time from `precmd` request to first prompt reply.
- Time from first reply to async update.
- Redraw count per prompt generation.
