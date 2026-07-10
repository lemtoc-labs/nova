window.BENCHMARK_DATA = {
  "lastUpdate": 1783688178820,
  "repoUrl": "https://github.com/lemtoc-labs/nova",
  "entries": {
    "Zsh Interactive Latency": [
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "564538d5002c3b5589d95336775e50ba75e71e11",
          "message": "fix(ci): run self-hosted benchmark in Nix shell (#3)\n\n- Use the flake dev shell for release builds and zsh-bench on the self-hosted runner\n- Store fixed-machine benchmark history under bench/zsh on gh-pages\n- Document the Pages dashboard location for self-hosted benchmark results\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-07T10:31:37+09:00",
          "tree_id": "2ae6ee565c14188f38e76737ca72b1b3c2080267",
          "url": "https://github.com/lemtoc-labs/nova/commit/564538d5002c3b5589d95336775e50ba75e71e11"
        },
        "date": 1783387944967,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 28.61,
            "range": "2.55 ms",
            "unit": "ms",
            "extra": "median: 28.61 ms\nmin: 28.14 ms\nmax: 38.58 ms\nstddev: 2.55 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 28.85,
            "range": "2.55 ms",
            "unit": "ms",
            "extra": "median: 28.85 ms\nmin: 28.39 ms\nmax: 38.84 ms\nstddev: 2.55 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.41,
            "range": "0.01 ms",
            "unit": "ms",
            "extra": "median: 0.41 ms\nmin: 0.35 ms\nmax: 0.42 ms\nstddev: 0.01 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.56,
            "range": "0.17 ms",
            "unit": "ms",
            "extra": "median: 0.56 ms\nmin: 0.32 ms\nmax: 0.8 ms\nstddev: 0.17 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 12.91,
            "range": "1.84 ms",
            "unit": "ms",
            "extra": "median: 12.91 ms\nmin: 12.31 ms\nmax: 20.42 ms\nstddev: 1.84 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "de46538f3027b23bf7c2fb8b82367868a7fd942f",
          "message": "chore(ci): customize zsh benchmark dashboard (#4)\n\n* chore(ci): customize zsh benchmark dashboard\n\n- Add a custom zsh-bench dashboard with summary, startup, interactive, and exit sections\n- Publish the dashboard template into the gh-pages benchmark directory after benchmark data is stored\n- Add a local preview script for checking the dashboard before publishing\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* chore(ci): keep dashboard preview under repo output\n\n- Write local dashboard previews to the ignored .preview/zsh-bench-dashboard directory\n- Keep preview file URLs stable across repeated checks\n- Document the ignored preview output path\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* fix(test): avoid script execution race\n\n- Write generated test scripts through an explicit file handle\n- Sync and close scripts before marking them executable\n- Avoid intermittent Text file busy failures on Linux CI\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* fix(test): sync generated script metadata\n\n- Reopen generated test scripts after chmod so metadata is flushed before execution\n- Avoid intermittent Text file busy failures on Linux CI\n- Keep the change scoped to the Unix test helper\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-07T11:16:00+09:00",
          "tree_id": "afccfbb1120af100c6e7140b2a54ff74179186dd",
          "url": "https://github.com/lemtoc-labs/nova/commit/de46538f3027b23bf7c2fb8b82367868a7fd942f"
        },
        "date": 1783390608764,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 28.58,
            "range": "2.66 ms",
            "unit": "ms",
            "extra": "median: 28.58 ms\nmin: 23.55 ms\nmax: 36.1 ms\nstddev: 2.66 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 28.72,
            "range": "2.67 ms",
            "unit": "ms",
            "extra": "median: 28.72 ms\nmin: 23.8 ms\nmax: 36.33 ms\nstddev: 2.67 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.41,
            "range": "0.04 ms",
            "unit": "ms",
            "extra": "median: 0.41 ms\nmin: 0.31 ms\nmax: 0.42 ms\nstddev: 0.04 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.24,
            "range": "0.25 ms",
            "unit": "ms",
            "extra": "median: 0.24 ms\nmin: 0.1 ms\nmax: 0.87 ms\nstddev: 0.25 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 13.87,
            "range": "0.87 ms",
            "unit": "ms",
            "extra": "median: 13.87 ms\nmin: 12.72 ms\nmax: 15.59 ms\nstddev: 0.87 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4cc893f0289a701e52e9a48dce83b633d51e5753",
          "message": "feat(prompt): customize context layout (#5)\n\n* feat(prompt): customize context layout\n\n- Add time and user host context segments to the prompt renderer\n- Shorten home-relative paths and render command duration with a plus prefix\n- Support configurable prompt characters with failure-aware styling\n- Update zsh protocol, CLI preview output, and tests for the new layout\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* docs(config): add complete config example\n\n- Document every current layout segment and common segment option\n- Show icon, style, timeout, TTL, and prompt character overrides\n- Add a config parse test that keeps the example valid\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* feat(duration): support custom prefix styling\n\n- Add duration prefix and prefix_style configuration\n- Render duration prefixes and values as separately styled parts\n- Support bright ANSI colors for terminals that render white as gray\n- Update the complete config example and parser coverage\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-07T16:11:06+09:00",
          "tree_id": "4bec875a55a059545a787bff6c77ec6444dcba4a",
          "url": "https://github.com/lemtoc-labs/nova/commit/4cc893f0289a701e52e9a48dce83b633d51e5753"
        },
        "date": 1783408313652,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.56,
            "range": "2.9 ms",
            "unit": "ms",
            "extra": "median: 29.56 ms\nmin: 27.95 ms\nmax: 40.72 ms\nstddev: 2.9 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.7,
            "range": "2.91 ms",
            "unit": "ms",
            "extra": "median: 29.7 ms\nmin: 28.1 ms\nmax: 40.89 ms\nstddev: 2.91 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.44,
            "range": "0.03 ms",
            "unit": "ms",
            "extra": "median: 0.44 ms\nmin: 0.38 ms\nmax: 0.49 ms\nstddev: 0.03 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.65,
            "range": "0.24 ms",
            "unit": "ms",
            "extra": "median: 0.65 ms\nmin: 0.17 ms\nmax: 0.88 ms\nstddev: 0.24 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 13.83,
            "range": "0.47 ms",
            "unit": "ms",
            "extra": "median: 13.83 ms\nmin: 12.86 ms\nmax: 14.53 ms\nstddev: 0.47 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "54d2e44092458094c397a8f9c5e140278edb906c",
          "message": "chore(release): add distribution setup (#6)\n\n* chore(release): add distribution setup\n\n- Configure cargo-dist release artifacts and Homebrew tap publishing\n- Add Nix package and app outputs for nix run/profile install\n- Document user install paths with English and Japanese README files\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* style(ci): format zsh benchmark dashboard\n\n- Apply oxfmt formatting to the zsh benchmark dashboard template\n- Align the performance threshold table formatting in the docs\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* fix(release): allow pinned release workflow\n\n- Mark cargo-dist CI generation as allow-dirty for the pinact-pinned release workflow\n- Update the contributor release validation command to run dist plan without the CLI override\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-07T17:09:09+09:00",
          "tree_id": "700ca4e423b8babaebf034d96fb5f648090ea0cb",
          "url": "https://github.com/lemtoc-labs/nova/commit/54d2e44092458094c397a8f9c5e140278edb906c"
        },
        "date": 1783411798843,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 28.67,
            "range": "1.98 ms",
            "unit": "ms",
            "extra": "median: 28.67 ms\nmin: 23.94 ms\nmax: 31.24 ms\nstddev: 1.98 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 28.82,
            "range": "1.99 ms",
            "unit": "ms",
            "extra": "median: 28.82 ms\nmin: 24.08 ms\nmax: 31.43 ms\nstddev: 1.99 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.42,
            "range": "0.03 ms",
            "unit": "ms",
            "extra": "median: 0.42 ms\nmin: 0.35 ms\nmax: 0.46 ms\nstddev: 0.03 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.42,
            "range": "0.19 ms",
            "unit": "ms",
            "extra": "median: 0.42 ms\nmin: 0.13 ms\nmax: 0.71 ms\nstddev: 0.19 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 13.93,
            "range": "1.02 ms",
            "unit": "ms",
            "extra": "median: 13.93 ms\nmin: 13.01 ms\nmax: 16.44 ms\nstddev: 1.02 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "85a5cc1136cda235986b48ec560263ebd1404f90",
          "message": "fix(config): use config.toml as default config path (#7)\n\n* fix(config): use config.toml as default config path\n\n- Change discovered config filenames from nova.toml to config.toml\n- Add unit coverage for NOVA_CONFIG, XDG_CONFIG_HOME, and HOME fallback discovery\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* docs(config): document config.toml setup\n\n- Update English and Japanese READMEs to reference config.toml\n- Replace the local example copy command with a raw GitHub download flow\n- Align the design note and complete example comments with the default lookup path\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-07T19:02:44+09:00",
          "tree_id": "d6aa6d42123a27e784fadd25931cd9a2d51bb8b1",
          "url": "https://github.com/lemtoc-labs/nova/commit/85a5cc1136cda235986b48ec560263ebd1404f90"
        },
        "date": 1783418612150,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.45,
            "range": "4.09 ms",
            "unit": "ms",
            "extra": "median: 29.45 ms\nmin: 26.84 ms\nmax: 43.03 ms\nstddev: 4.09 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.59,
            "range": "4.1 ms",
            "unit": "ms",
            "extra": "median: 29.59 ms\nmin: 26.98 ms\nmax: 43.23 ms\nstddev: 4.1 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.44,
            "range": "0.03 ms",
            "unit": "ms",
            "extra": "median: 0.44 ms\nmin: 0.4 ms\nmax: 0.53 ms\nstddev: 0.03 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.42,
            "range": "0.2 ms",
            "unit": "ms",
            "extra": "median: 0.42 ms\nmin: 0.15 ms\nmax: 0.84 ms\nstddev: 0.2 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 13.75,
            "range": "1.03 ms",
            "unit": "ms",
            "extra": "median: 13.75 ms\nmin: 13.11 ms\nmax: 16.45 ms\nstddev: 1.03 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "bf36f87d6d3b9fe8aff6967b20e5b4053808024e",
          "message": "chore(release): bump version to 0.1.1 (#8)\n\n- Update the crate version in Cargo.toml to 0.1.1\n- Refresh Cargo.lock so the package metadata matches the release version\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-07T19:18:56+09:00",
          "tree_id": "cf51238ef996d9f23101b675388dc52fda65c5b6",
          "url": "https://github.com/lemtoc-labs/nova/commit/bf36f87d6d3b9fe8aff6967b20e5b4053808024e"
        },
        "date": 1783419583880,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 28.54,
            "range": "3.25 ms",
            "unit": "ms",
            "extra": "median: 28.54 ms\nmin: 28.03 ms\nmax: 41.95 ms\nstddev: 3.25 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 28.67,
            "range": "3.25 ms",
            "unit": "ms",
            "extra": "median: 28.67 ms\nmin: 28.17 ms\nmax: 42.12 ms\nstddev: 3.25 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.42,
            "range": "0.06 ms",
            "unit": "ms",
            "extra": "median: 0.42 ms\nmin: 0.36 ms\nmax: 0.62 ms\nstddev: 0.06 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.6,
            "range": "0.14 ms",
            "unit": "ms",
            "extra": "median: 0.6 ms\nmin: 0.14 ms\nmax: 0.76 ms\nstddev: 0.14 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 13.05,
            "range": "0.45 ms",
            "unit": "ms",
            "extra": "median: 13.05 ms\nmin: 12.72 ms\nmax: 14.74 ms\nstddev: 0.45 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "ad47b84bb936ee128e779d8813ed3699d1f424a9",
          "message": "feat(aws): add segment formatting (#9)\n\n* feat(aws): add segment formatting\n\n- Add force_display and format settings for the AWS segment.\n- Support $symbol, $profile, $region, and optional format groups.\n- Document planned AWS credential duration support.\n- Isolate prompt tests from user config and shell environment.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* fix(ci): satisfy clippy for aws formatter\n\n- Collapse the optional group parser condition to satisfy clippy.\n- Keep the AWS formatter behavior unchanged.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-07T23:14:15+09:00",
          "tree_id": "aaf6b88c4c68f126e32cef90035d63b7dbb1ecfc",
          "url": "https://github.com/lemtoc-labs/nova/commit/ad47b84bb936ee128e779d8813ed3699d1f424a9"
        },
        "date": 1783433704951,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 28.87,
            "range": "1.62 ms",
            "unit": "ms",
            "extra": "median: 28.87 ms\nmin: 26.52 ms\nmax: 32.82 ms\nstddev: 1.62 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.06,
            "range": "1.62 ms",
            "unit": "ms",
            "extra": "median: 29.06 ms\nmin: 26.67 ms\nmax: 32.97 ms\nstddev: 1.62 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.43,
            "range": "0.02 ms",
            "unit": "ms",
            "extra": "median: 0.43 ms\nmin: 0.36 ms\nmax: 0.46 ms\nstddev: 0.02 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.5,
            "range": "0.21 ms",
            "unit": "ms",
            "extra": "median: 0.5 ms\nmin: 0.18 ms\nmax: 0.77 ms\nstddev: 0.21 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 13.51,
            "range": "1.47 ms",
            "unit": "ms",
            "extra": "median: 13.51 ms\nmin: 12.88 ms\nmax: 18.58 ms\nstddev: 1.47 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9af769f2cf3d90687898de38cf2ea78cb5d09aad",
          "message": "fix(runtime): propagate request PATH (#10)\n\n* fix(runtime): propagate request PATH\n\n- add PATH to the worker protocol and zsh adapter\n- apply request PATH to runtime version collectors and cache keys\n- document runtime lookup semantics and cover worker FIFO propagation\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* fix(runtime): satisfy protocol test clippy\n\n- replace an infallible destructuring match in protocol tests\n- keep the request PATH protocol coverage unchanged\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-08T00:04:33+09:00",
          "tree_id": "6fd868fbccd95511296f762c4719c3ff981025bd",
          "url": "https://github.com/lemtoc-labs/nova/commit/9af769f2cf3d90687898de38cf2ea78cb5d09aad"
        },
        "date": 1783436803028,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.28,
            "range": "3.62 ms",
            "unit": "ms",
            "extra": "median: 29.28 ms\nmin: 26.27 ms\nmax: 39.02 ms\nstddev: 3.62 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.53,
            "range": "3.59 ms",
            "unit": "ms",
            "extra": "median: 29.53 ms\nmin: 26.53 ms\nmax: 39.16 ms\nstddev: 3.59 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.46,
            "range": "0.03 ms",
            "unit": "ms",
            "extra": "median: 0.46 ms\nmin: 0.38 ms\nmax: 0.49 ms\nstddev: 0.03 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.61,
            "range": "0.13 ms",
            "unit": "ms",
            "extra": "median: 0.61 ms\nmin: 0.38 ms\nmax: 0.84 ms\nstddev: 0.13 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 13.42,
            "range": "1.04 ms",
            "unit": "ms",
            "extra": "median: 13.42 ms\nmin: 12.59 ms\nmax: 15.74 ms\nstddev: 1.04 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "e1a30e61d0e4bba05d2e655839bdb07a4a78b359",
          "message": "fix(worker): correct async pipeline behavior (#11)",
          "timestamp": "2026-07-08T00:38:13+09:00",
          "tree_id": "290589ca2f245007b3b94a6acd59a21b88980ee3",
          "url": "https://github.com/lemtoc-labs/nova/commit/e1a30e61d0e4bba05d2e655839bdb07a4a78b359"
        },
        "date": 1783438876760,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.11,
            "range": "3.41 ms",
            "unit": "ms",
            "extra": "median: 29.11 ms\nmin: 23.46 ms\nmax: 40.36 ms\nstddev: 3.41 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.35,
            "range": "3.42 ms",
            "unit": "ms",
            "extra": "median: 29.35 ms\nmin: 23.58 ms\nmax: 40.53 ms\nstddev: 3.42 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.45,
            "range": "0.03 ms",
            "unit": "ms",
            "extra": "median: 0.45 ms\nmin: 0.38 ms\nmax: 0.49 ms\nstddev: 0.03 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.61,
            "range": "0.12 ms",
            "unit": "ms",
            "extra": "median: 0.61 ms\nmin: 0.34 ms\nmax: 0.8 ms\nstddev: 0.12 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 13.13,
            "range": "0.51 ms",
            "unit": "ms",
            "extra": "median: 13.13 ms\nmin: 12.68 ms\nmax: 14.29 ms\nstddev: 0.51 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "70647ade07471e48b183be20042ad789942bffc9",
          "message": "feat(worker): add initial render wait budget (#12)",
          "timestamp": "2026-07-08T01:02:47+09:00",
          "tree_id": "4d5d24d31085f4ef30140e207443edeb6a7bf95b",
          "url": "https://github.com/lemtoc-labs/nova/commit/70647ade07471e48b183be20042ad789942bffc9"
        },
        "date": 1783440375233,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 22.91,
            "range": "1.09 ms",
            "unit": "ms",
            "extra": "median: 22.91 ms\nmin: 22.02 ms\nmax: 26.66 ms\nstddev: 1.09 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 23.04,
            "range": "1.09 ms",
            "unit": "ms",
            "extra": "median: 23.04 ms\nmin: 22.15 ms\nmax: 26.81 ms\nstddev: 1.09 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.47,
            "range": "0.01 ms",
            "unit": "ms",
            "extra": "median: 0.47 ms\nmin: 0.45 ms\nmax: 0.5 ms\nstddev: 0.01 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.63,
            "range": "0.1 ms",
            "unit": "ms",
            "extra": "median: 0.63 ms\nmin: 0.43 ms\nmax: 0.8 ms\nstddev: 0.1 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 14.15,
            "range": "0.38 ms",
            "unit": "ms",
            "extra": "median: 14.15 ms\nmin: 13.33 ms\nmax: 14.77 ms\nstddev: 0.38 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "29f2ebe59a2793b082091924188bf49779787830",
          "message": "perf(worker): keep initial wait default at zero (#13)",
          "timestamp": "2026-07-08T01:18:20+09:00",
          "tree_id": "9fa4e2c9d93e7f05713adbc20b9ba4e75d43f031",
          "url": "https://github.com/lemtoc-labs/nova/commit/29f2ebe59a2793b082091924188bf49779787830"
        },
        "date": 1783443421634,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 30.64,
            "range": "2.75 ms",
            "unit": "ms",
            "extra": "median: 30.64 ms\nmin: 27.75 ms\nmax: 38.08 ms\nstddev: 2.75 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 30.78,
            "range": "2.78 ms",
            "unit": "ms",
            "extra": "median: 30.78 ms\nmin: 27.89 ms\nmax: 38.32 ms\nstddev: 2.78 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.46,
            "range": "0.03 ms",
            "unit": "ms",
            "extra": "median: 0.46 ms\nmin: 0.41 ms\nmax: 0.52 ms\nstddev: 0.03 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.48,
            "range": "0.2 ms",
            "unit": "ms",
            "extra": "median: 0.48 ms\nmin: 0.16 ms\nmax: 0.81 ms\nstddev: 0.2 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 14.43,
            "range": "1.03 ms",
            "unit": "ms",
            "extra": "median: 14.43 ms\nmin: 12.98 ms\nmax: 16.22 ms\nstddev: 1.03 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "lemtoc",
            "username": "lemtoc"
          },
          "committer": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "lemtoc",
            "username": "lemtoc"
          },
          "distinct": true,
          "id": "844a135d8a6c8fb75268ee8abca78c9074d0fbcb",
          "message": "fix(ci): disable benchmark commit signing\n\n- Disable local git commit signing before github-action-benchmark writes benchmark data.\n- Match the dotfiles benchmark workflow so self-hosted runs do not depend on interactive 1Password signing.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-08T01:55:52+09:00",
          "tree_id": "24171566e5ead1c2161bdc448427d0cb2810141c",
          "url": "https://github.com/lemtoc-labs/nova/commit/844a135d8a6c8fb75268ee8abca78c9074d0fbcb"
        },
        "date": 1783443474699,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.44,
            "range": "2.84 ms",
            "unit": "ms",
            "extra": "median: 29.44 ms\nmin: 28.32 ms\nmax: 38.37 ms\nstddev: 2.84 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.64,
            "range": "2.82 ms",
            "unit": "ms",
            "extra": "median: 29.64 ms\nmin: 28.56 ms\nmax: 38.56 ms\nstddev: 2.82 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.46,
            "range": "0.02 ms",
            "unit": "ms",
            "extra": "median: 0.46 ms\nmin: 0.41 ms\nmax: 0.53 ms\nstddev: 0.02 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.6,
            "range": "0.16 ms",
            "unit": "ms",
            "extra": "median: 0.6 ms\nmin: 0.17 ms\nmax: 0.77 ms\nstddev: 0.16 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 13.29,
            "range": "0.94 ms",
            "unit": "ms",
            "extra": "median: 13.29 ms\nmin: 12.86 ms\nmax: 16.06 ms\nstddev: 0.94 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4fc54d252b68b9b94b0265eca72cc9c45805e70d",
          "message": "perf(worker): batch internal quality fixes (#14)\n\n* fix(worker): detect partial request writes\n\n- Move libc to dev-dependencies because it is only used by tests.\n- Check syswrite byte counts so partial request writes force the worker-dead fallback path.\n- Cover the generated zsh init script shape for the write-count check.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* perf(worker): gate config reloads by fingerprint\n\n- Track the discovered config path and mtime/size fingerprint in worker state.\n- Reparse config only when the fingerprint changes and share active config with Arc.\n- Return borrowed segment config for render paths and clone only for spawned job closures.\n- Document that config discovery environment is fixed at worker startup.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* perf(worker): evict cache entries by lru\n\n- Track last_used on cache entries and touch them during lookup.\n- Evict the least recently used entry when capacity is reached.\n- Update worker cache access and design docs for mutable lookups.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* perf(worker): eager spawn zsh worker\n\n- Spawn the worker during interactive zsh init while avoiding duplicate spawns on the first prompt.\n- Clean up an eagerly spawned worker on shell exit before removing its runtime directory.\n- Record the zsh-bench first prompt measurement and document the stable result.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-08T02:20:07+09:00",
          "tree_id": "1121da76fda28f102351715d20482cf952bc5fd0",
          "url": "https://github.com/lemtoc-labs/nova/commit/4fc54d252b68b9b94b0265eca72cc9c45805e70d"
        },
        "date": 1783445630449,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.68,
            "range": "3.34 ms",
            "unit": "ms",
            "extra": "median: 29.68 ms\nmin: 26.51 ms\nmax: 42.18 ms\nstddev: 3.34 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.82,
            "range": "3.34 ms",
            "unit": "ms",
            "extra": "median: 29.82 ms\nmin: 26.62 ms\nmax: 42.34 ms\nstddev: 3.34 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.51,
            "range": "0.03 ms",
            "unit": "ms",
            "extra": "median: 0.51 ms\nmin: 0.44 ms\nmax: 0.54 ms\nstddev: 0.03 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.61,
            "range": "0.2 ms",
            "unit": "ms",
            "extra": "median: 0.61 ms\nmin: 0.24 ms\nmax: 0.89 ms\nstddev: 0.2 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 21.21,
            "range": "0.99 ms",
            "unit": "ms",
            "extra": "median: 21.21 ms\nmin: 20.32 ms\nmax: 23.71 ms\nstddev: 0.99 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "0369c0d118349d225904b2619c64dc4601494691",
          "message": "feat(config): add renderer customization options (#15)\n\n* feat(config): support extended color formats\n\n- Lower ANSI 256-color indexes and truecolor hex strings to SGR sequences.\n- Keep named color output unchanged and ignore invalid colors during lowering.\n- Report invalid configured colors through the existing warning model.\n- Document the supported color formats in design and example config docs.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* feat(config): allow custom segment separators\n\n- Add an optional layout separator with the existing space separator as the default.\n- Apply the separator consistently when lowering prompt sides and calculating widths.\n- Cover custom separator snapshots and width invariants.\n- Document the separator option in design and example config docs.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* feat(config): add async loading placeholders\n\n- Add an opt-in loading placeholder field to segment config.\n- Render configured loading placeholders with the segment style while keeping default Loading hidden.\n- Keep render status partial while placeholders are displayed.\n- Document the loading option in design and example config docs.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-08T02:36:09+09:00",
          "tree_id": "343f236f1ca94ee3917b1c7a6ebc9e767a0eee81",
          "url": "https://github.com/lemtoc-labs/nova/commit/0369c0d118349d225904b2619c64dc4601494691"
        },
        "date": 1783445900674,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 28.87,
            "range": "2.82 ms",
            "unit": "ms",
            "extra": "median: 28.87 ms\nmin: 27.87 ms\nmax: 38.34 ms\nstddev: 2.82 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.04,
            "range": "2.82 ms",
            "unit": "ms",
            "extra": "median: 29.04 ms\nmin: 28 ms\nmax: 38.57 ms\nstddev: 2.82 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.49,
            "range": "0.02 ms",
            "unit": "ms",
            "extra": "median: 0.49 ms\nmin: 0.47 ms\nmax: 0.54 ms\nstddev: 0.02 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.77,
            "range": "0.12 ms",
            "unit": "ms",
            "extra": "median: 0.77 ms\nmin: 0.54 ms\nmax: 0.9 ms\nstddev: 0.12 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 20.54,
            "range": "0.49 ms",
            "unit": "ms",
            "extra": "median: 20.54 ms\nmin: 20.07 ms\nmax: 22.17 ms\nstddev: 0.49 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8e9ff54fedd3636cc3c5ea02b30743305212ed23",
          "message": "refactor(segments): introduce async segment registry (#16)\n\n- Move known segment ids out of config and into the segment registry.\n- Add async segment specs for git and runtime collectors.\n- Replace per-segment worker scheduling and lookup functions with registry loops.\n- Carry panic failure keys through jobs so multi-output segments fail consistently.\n- Share common node, bun, and deno command collection logic.\n- Document the async segment registry shape and addition steps.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-08T02:49:42+09:00",
          "tree_id": "89e556ae2e21586dc689ff2206c46eaa72d39c64",
          "url": "https://github.com/lemtoc-labs/nova/commit/8e9ff54fedd3636cc3c5ea02b30743305212ed23"
        },
        "date": 1783446739332,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 28.87,
            "range": "3.15 ms",
            "unit": "ms",
            "extra": "median: 28.87 ms\nmin: 25.43 ms\nmax: 40.49 ms\nstddev: 3.15 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29,
            "range": "3.21 ms",
            "unit": "ms",
            "extra": "median: 29 ms\nmin: 25.56 ms\nmax: 40.65 ms\nstddev: 3.21 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.49,
            "range": "0.06 ms",
            "unit": "ms",
            "extra": "median: 0.49 ms\nmin: 0.43 ms\nmax: 0.66 ms\nstddev: 0.06 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.63,
            "range": "0.19 ms",
            "unit": "ms",
            "extra": "median: 0.63 ms\nmin: 0.21 ms\nmax: 0.87 ms\nstddev: 0.19 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 21.75,
            "range": "2.54 ms",
            "unit": "ms",
            "extra": "median: 21.75 ms\nmin: 19.97 ms\nmax: 30.53 ms\nstddev: 2.54 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "63730f30c85da96d2c792b895c240a54e871698d",
          "message": "chore(release): bump version to 0.2.0 (#17)\n\n- Update the package version in Cargo.toml.\n- Update the nova package version in Cargo.lock.\n- Prepare the next cargo-dist release tag.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-08T10:23:10+09:00",
          "tree_id": "57037a785132011ad4037eed63c728cd0c25172c",
          "url": "https://github.com/lemtoc-labs/nova/commit/63730f30c85da96d2c792b895c240a54e871698d"
        },
        "date": 1783473836269,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.31,
            "range": "13.64 ms",
            "unit": "ms",
            "extra": "median: 29.31 ms\nmin: 27.41 ms\nmax: 79.18 ms\nstddev: 13.64 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.45,
            "range": "13.77 ms",
            "unit": "ms",
            "extra": "median: 29.45 ms\nmin: 27.54 ms\nmax: 79.91 ms\nstddev: 13.77 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.53,
            "range": "0.25 ms",
            "unit": "ms",
            "extra": "median: 0.53 ms\nmin: 0.4 ms\nmax: 1.24 ms\nstddev: 0.25 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.33,
            "range": "0.18 ms",
            "unit": "ms",
            "extra": "median: 0.33 ms\nmin: 0.13 ms\nmax: 0.78 ms\nstddev: 0.18 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 22.21,
            "range": "16.59 ms",
            "unit": "ms",
            "extra": "median: 22.21 ms\nmin: 19.46 ms\nmax: 79.62 ms\nstddev: 16.59 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "852dcc160aa2d4a2547698437a13d2010b51d2fe",
          "message": "docs(readme): add async prompt recording (#18)\n\n* docs(readme): add async prompt recording\n\n- Embed a generated VHS prompt recording in the English and Japanese READMEs.\n- Add reusable VHS tape assets that demonstrate async Git and Rust prompt updates.\n- Add a fixed VHS wrapper to the Nix dev shell and document regeneration.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* chore(repo): organize support assets\n\n- Move README VHS recording assets under docs/assets.\n- Move the embedded zsh init script under src/shell with its Rust include path.\n- Apply markdown table formatting to design and performance docs.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-08T15:30:34+09:00",
          "tree_id": "8956bd498ede9498503fa30185d570c77dc78da2",
          "url": "https://github.com/lemtoc-labs/nova/commit/852dcc160aa2d4a2547698437a13d2010b51d2fe"
        },
        "date": 1783572712984,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 32.74,
            "range": "3.6 ms",
            "unit": "ms",
            "extra": "median: 32.74 ms\nmin: 27.84 ms\nmax: 40.76 ms\nstddev: 3.6 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 32.89,
            "range": "3.61 ms",
            "unit": "ms",
            "extra": "median: 32.89 ms\nmin: 27.98 ms\nmax: 40.89 ms\nstddev: 3.61 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.59,
            "range": "0.05 ms",
            "unit": "ms",
            "extra": "median: 0.59 ms\nmin: 0.53 ms\nmax: 0.73 ms\nstddev: 0.05 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.56,
            "range": "0.14 ms",
            "unit": "ms",
            "extra": "median: 0.56 ms\nmin: 0.29 ms\nmax: 0.74 ms\nstddev: 0.14 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 23.47,
            "range": "3.03 ms",
            "unit": "ms",
            "extra": "median: 23.47 ms\nmin: 19.74 ms\nmax: 30.39 ms\nstddev: 3.03 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "3296618a613eae6fa26b3bf12a50ec18afb0dfc9",
          "message": "test(worker): isolate worker config in integration tests (#45)\n\n- Add a shared worker spawn helper that always sets isolated HOME and XDG_CONFIG_HOME.\n- Route worker integration tests through explicit default or fixture config selection.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-09T13:32:42+09:00",
          "tree_id": "14e226b0aa9bd8640e83962657beb1318bdd62a3",
          "url": "https://github.com/lemtoc-labs/nova/commit/3296618a613eae6fa26b3bf12a50ec18afb0dfc9"
        },
        "date": 1783572769395,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 32,
            "range": "3.56 ms",
            "unit": "ms",
            "extra": "median: 32 ms\nmin: 27.24 ms\nmax: 43.03 ms\nstddev: 3.56 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 32.15,
            "range": "3.52 ms",
            "unit": "ms",
            "extra": "median: 32.15 ms\nmin: 27.38 ms\nmax: 43.2 ms\nstddev: 3.52 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.59,
            "range": "0.25 ms",
            "unit": "ms",
            "extra": "median: 0.59 ms\nmin: 0.5 ms\nmax: 1.62 ms\nstddev: 0.25 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.7,
            "range": "0.18 ms",
            "unit": "ms",
            "extra": "median: 0.7 ms\nmin: 0.31 ms\nmax: 0.9 ms\nstddev: 0.18 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 23.36,
            "range": "2.3 ms",
            "unit": "ms",
            "extra": "median: 23.36 ms\nmin: 19.93 ms\nmax: 25.99 ms\nstddev: 2.3 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "c08748c16aa2a6845ce58a0fd70f907ebecb2c9d",
          "message": "fix(worker): harden runtime directory and lifecycle (#46)\n\n* fix(worker): harden runtime directory and lifecycle\n\n- Create zsh runtime directories and FIFOs with private permissions.\n- Pass worker session details through environment variables instead of argv.\n- Add parent-process monitoring so orphaned workers exit and clean up runtime dirs.\n- Cover runtime hardening and pre-transport orphan cleanup with tests.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* perf(shell): avoid zsh-side token generation\n\n- Generate the zsh session token from /dev/urandom while rendering nova init zsh instead of spawning od and tr during shell startup.\n- Keep runtime hardening with explicit mkdir -m 700 and mkfifo -m 600 while avoiding extra chmod and umask work.\n- Add zsh init coverage for token embedding and hardened FIFO creation.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-09T15:58:56+09:00",
          "tree_id": "702b3907ffc34d66a7b3dd4e1a0b87af62db833b",
          "url": "https://github.com/lemtoc-labs/nova/commit/c08748c16aa2a6845ce58a0fd70f907ebecb2c9d"
        },
        "date": 1783580385003,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 31.04,
            "range": "1.65 ms",
            "unit": "ms",
            "extra": "median: 31.04 ms\nmin: 28.33 ms\nmax: 34.29 ms\nstddev: 1.65 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 31.18,
            "range": "1.65 ms",
            "unit": "ms",
            "extra": "median: 31.18 ms\nmin: 28.47 ms\nmax: 34.44 ms\nstddev: 1.65 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.6,
            "range": "0.11 ms",
            "unit": "ms",
            "extra": "median: 0.6 ms\nmin: 0.51 ms\nmax: 1.01 ms\nstddev: 0.11 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.51,
            "range": "0.15 ms",
            "unit": "ms",
            "extra": "median: 0.51 ms\nmin: 0.29 ms\nmax: 0.79 ms\nstddev: 0.15 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 23.53,
            "range": "1.77 ms",
            "unit": "ms",
            "extra": "median: 23.53 ms\nmin: 21.83 ms\nmax: 26.92 ms\nstddev: 1.77 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "b555f301e7ab0e3ac0b13bcef3cfc083bfcbdf3a",
          "message": "fix(worker): send final status updates (#47)\n\n- Track the last sent render status on the active prompt.\n- Send worker updates when status changes even if the rendered prompt text is unchanged.\n- Cover clean git status, missing rust command, and initial git failure final-status transitions.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-09T18:38:34+09:00",
          "tree_id": "dc483868026ca5e90aa6462a89ab0220cab7069b",
          "url": "https://github.com/lemtoc-labs/nova/commit/b555f301e7ab0e3ac0b13bcef3cfc083bfcbdf3a"
        },
        "date": 1783589962975,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 30.44,
            "range": "1.85 ms",
            "unit": "ms",
            "extra": "median: 30.44 ms\nmin: 28.59 ms\nmax: 36.88 ms\nstddev: 1.85 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 30.6,
            "range": "1.86 ms",
            "unit": "ms",
            "extra": "median: 30.6 ms\nmin: 28.73 ms\nmax: 37.06 ms\nstddev: 1.86 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.57,
            "range": "0.03 ms",
            "unit": "ms",
            "extra": "median: 0.57 ms\nmin: 0.53 ms\nmax: 0.66 ms\nstddev: 0.03 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.6,
            "range": "0.18 ms",
            "unit": "ms",
            "extra": "median: 0.6 ms\nmin: 0.23 ms\nmax: 0.83 ms\nstddev: 0.18 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 21.93,
            "range": "1.57 ms",
            "unit": "ms",
            "extra": "median: 21.93 ms\nmin: 20.3 ms\nmax: 26.44 ms\nstddev: 1.57 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7092672410cee80653d9e123f536386c019858c3",
          "message": "feat(bench): compare zsh-bench against stored main (#49)\n\n- Add a gh-pages baseline extractor for stored zsh-bench history.\n- Render Previous and Delta columns when a previous result is available.\n- Wire PR and main benchmark workflows to load stored baselines without a second benchmark run.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-09T18:58:28+09:00",
          "tree_id": "7725174d5e43d6b3919018143d6ca9685e9dcf0f",
          "url": "https://github.com/lemtoc-labs/nova/commit/7092672410cee80653d9e123f536386c019858c3"
        },
        "date": 1783591158191,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 31.78,
            "range": "2.44 ms",
            "unit": "ms",
            "extra": "median: 31.78 ms\nmin: 29.11 ms\nmax: 38.54 ms\nstddev: 2.44 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 31.97,
            "range": "2.44 ms",
            "unit": "ms",
            "extra": "median: 31.97 ms\nmin: 29.25 ms\nmax: 38.7 ms\nstddev: 2.44 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.59,
            "range": "0.06 ms",
            "unit": "ms",
            "extra": "median: 0.59 ms\nmin: 0.48 ms\nmax: 0.76 ms\nstddev: 0.06 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.6,
            "range": "0.17 ms",
            "unit": "ms",
            "extra": "median: 0.6 ms\nmin: 0.23 ms\nmax: 0.81 ms\nstddev: 0.17 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 23.11,
            "range": "2.23 ms",
            "unit": "ms",
            "extra": "median: 23.11 ms\nmin: 20.51 ms\nmax: 28.96 ms\nstddev: 2.23 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "da427d4a93bb3f520274716da9e9e8741c124936",
          "message": "fix(zsh): rerender prompt on keymap changes (#50)\n\n- Store the last prompt exit status and command duration for zle-triggered rerenders.\n- Register a keymap-select hook that asks the worker to render with the current zsh keymap.\n- Cover the generated init script wiring for vi prompt character updates.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-09T19:07:24+09:00",
          "tree_id": "d36e3a1c0a0413ac870f32d69bfc12cd3df602d3",
          "url": "https://github.com/lemtoc-labs/nova/commit/da427d4a93bb3f520274716da9e9e8741c124936"
        },
        "date": 1783591784278,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.13,
            "range": "1.93 ms",
            "unit": "ms",
            "extra": "median: 29.13 ms\nmin: 28.67 ms\nmax: 36.93 ms\nstddev: 1.93 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.27,
            "range": "1.93 ms",
            "unit": "ms",
            "extra": "median: 29.27 ms\nmin: 28.81 ms\nmax: 37.09 ms\nstddev: 1.93 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.56,
            "range": "0.01 ms",
            "unit": "ms",
            "extra": "median: 0.56 ms\nmin: 0.53 ms\nmax: 0.58 ms\nstddev: 0.01 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.71,
            "range": "0.22 ms",
            "unit": "ms",
            "extra": "median: 0.71 ms\nmin: 0.16 ms\nmax: 0.95 ms\nstddev: 0.22 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 20.72,
            "range": "0.43 ms",
            "unit": "ms",
            "extra": "median: 20.72 ms\nmin: 20 ms\nmax: 21.69 ms\nstddev: 0.43 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "eac6676f828f501937fbc35a743213a4667efc94",
          "message": "fix(render): prioritize narrow prompt segments (#51)\n\n* fix(render): preserve dir when fitting narrow prompts\n\n- Keep the directory segment at least at its last path component while fitting.\n- Drop overflowing segments by built-in importance instead of tail position.\n- Remove zero-width fitted segments so they do not leave separator gaps.\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* fix(render): fit narrow prompts without dropping segments\n\n- Preserve all rendered segments during fitting instead of dropping low-priority prompt data\n- Compact only truncatable content: dir paths, branch labels, git status counts, and runtime labels\n- Reserve command input space before deciding whether an input prompt still fits on one line\n- Ellipsize the left side fish-style when fitted prompt content still exceeds the input budget\n- Move prompt_char onto the final input line so the prompt remains usable even in narrow terminals\n- Hide rprompt only when the input prompt wraps, avoiding overlap with the guaranteed input line\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* fix(render): guard input rprompt fitting\n\n- Hide input rprompt when it would collide with the left prompt\n\n- Truncate oversized input rprompt content to the terminal width before lowering\n\n- Preserve part-derived styles when truncating multipart segments\n\n- Treat escaped percent markers as literal percent signs in visible width calculations\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* fix(render): cap command input reservation\n\n- Limit command input reservation to 60 columns on wide terminals\n\n- Keep the existing 50 percent reservation behavior through 120 columns\n\n- Cover wide terminal prompt budget calculations in the render tests\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>\n\n* fix(render): prioritize narrow prompt segments\n\n- fit line segments to their actual available width and reserve space before line1 right prompts\n- hide narrow line1 right prompts and preserve primary git context before secondary runtime data\n- retain multipart segment styles when truncating rendered prompt text\n\nCo-authored-by: Codex GPT-5.6 Terra Xhigh <noreply@openai.com>\n\n* fix(render): preserve single-cell ellipsis\n\n- keep the omission marker visible when the first retained segment occupies one display cell\n- preserve multipart segment styling when replacing that cell with an ellipsis\n\nCo-authored-by: Codex GPT-5.6 Sol Ultra <noreply@openai.com>\n\n* fix(render): guarantee aligned side widths\n\n- truncate the widest non-prompt left segment until it fits the assigned columns\n- preserve rightmost right-prompt context and wrapped prompt-char input lines\n- cover mixed custom segments and align fitting test names\n\nCo-authored-by: Codex GPT-5.6 Sol Ultra <noreply@openai.com>\n\n* refactor(render): clarify non-prompt truncation\n\n- name the widest-segment fallback after its non-prompt contract\n- remove the unreachable prompt-char fallback and match branch\n\nCo-authored-by: Codex GPT-5.6 Sol Ultra <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.5 <noreply@openai.com>",
          "timestamp": "2026-07-10T17:03:36+09:00",
          "tree_id": "6551ae42987485aa0597245613ad4a8fd37ccba8",
          "url": "https://github.com/lemtoc-labs/nova/commit/eac6676f828f501937fbc35a743213a4667efc94"
        },
        "date": 1783671936429,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 30.58,
            "range": "3.01 ms",
            "unit": "ms",
            "extra": "median: 30.58 ms\nmin: 26.82 ms\nmax: 41.79 ms\nstddev: 3.01 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 30.71,
            "range": "3.02 ms",
            "unit": "ms",
            "extra": "median: 30.71 ms\nmin: 26.94 ms\nmax: 41.95 ms\nstddev: 3.02 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.58,
            "range": "0.02 ms",
            "unit": "ms",
            "extra": "median: 0.58 ms\nmin: 0.54 ms\nmax: 0.62 ms\nstddev: 0.02 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.4,
            "range": "0.18 ms",
            "unit": "ms",
            "extra": "median: 0.4 ms\nmin: 0.28 ms\nmax: 0.85 ms\nstddev: 0.18 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 21.18,
            "range": "1.17 ms",
            "unit": "ms",
            "extra": "median: 21.18 ms\nmin: 20.29 ms\nmax: 25.29 ms\nstddev: 1.17 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a0575d28ffe9acde05294dc3ad3c2f9426d39c93",
          "message": "fix(bench): deduplicate dashboard points (#52)\n\n- render only the newest dated benchmark result per commit within each suite\n- preserve every benchmark attempt in raw JSON downloads\n\nCo-authored-by: Codex GPT-5.6 Sol Ultra <noreply@openai.com>",
          "timestamp": "2026-07-10T17:47:49+09:00",
          "tree_id": "ca05d9ddc00d2a875a33c5759aaf1fc152e71b02",
          "url": "https://github.com/lemtoc-labs/nova/commit/a0575d28ffe9acde05294dc3ad3c2f9426d39c93"
        },
        "date": 1783673318925,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 31.34,
            "range": "3.06 ms",
            "unit": "ms",
            "extra": "median: 31.34 ms\nmin: 29.07 ms\nmax: 43.04 ms\nstddev: 3.06 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 31.48,
            "range": "3.05 ms",
            "unit": "ms",
            "extra": "median: 31.48 ms\nmin: 29.21 ms\nmax: 43.21 ms\nstddev: 3.05 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.6,
            "range": "0.06 ms",
            "unit": "ms",
            "extra": "median: 0.6 ms\nmin: 0.49 ms\nmax: 0.73 ms\nstddev: 0.06 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.49,
            "range": "0.2 ms",
            "unit": "ms",
            "extra": "median: 0.49 ms\nmin: 0.15 ms\nmax: 0.98 ms\nstddev: 0.2 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 21.73,
            "range": "0.98 ms",
            "unit": "ms",
            "extra": "median: 21.73 ms\nmin: 20.72 ms\nmax: 25.01 ms\nstddev: 0.98 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7e9127a65c8937831409b12e001d6bc8583a5120",
          "message": "chore(release): bump version to 0.3.1 (#53)\n\n- Update Cargo metadata and lockfile to 0.3.1\n- Align the Nix package version with the release version\n- Prepare the next cargo-dist release tag\n\nCo-authored-by: Codex GPT-5.6 Sol Ultra <noreply@openai.com>",
          "timestamp": "2026-07-10T20:15:09+09:00",
          "tree_id": "6846b56ef6959986497b570fd04fc45e7eff7893",
          "url": "https://github.com/lemtoc-labs/nova/commit/7e9127a65c8937831409b12e001d6bc8583a5120"
        },
        "date": 1783682160071,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.84,
            "range": "2.12 ms",
            "unit": "ms",
            "extra": "median: 29.84 ms\nmin: 27.76 ms\nmax: 36.61 ms\nstddev: 2.12 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.98,
            "range": "2.14 ms",
            "unit": "ms",
            "extra": "median: 29.98 ms\nmin: 27.93 ms\nmax: 36.86 ms\nstddev: 2.14 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.57,
            "range": "0.03 ms",
            "unit": "ms",
            "extra": "median: 0.57 ms\nmin: 0.51 ms\nmax: 0.6 ms\nstddev: 0.03 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.58,
            "range": "0.15 ms",
            "unit": "ms",
            "extra": "median: 0.58 ms\nmin: 0.33 ms\nmax: 0.85 ms\nstddev: 0.15 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 22.07,
            "range": "1.48 ms",
            "unit": "ms",
            "extra": "median: 22.07 ms\nmin: 20.91 ms\nmax: 26.7 ms\nstddev: 1.48 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "cdf4195d7b0498d32fa7c4483c181a15522cb866",
          "message": "feat(cli): add version and successful prompt help (#54)\n\n- Print the package version for --version and -V and advertise the flags in top-level help\n- Treat prompt --help and -h as successful stdout usage requests\n- Cover aliases, output streams, and exit status with CLI integration tests\n\nCo-authored-by: Codex GPT-5.6 Sol Ultra <noreply@openai.com>",
          "timestamp": "2026-07-10T20:44:23+09:00",
          "tree_id": "b24db2cf0f2fdc7ca3b681e30a6e61e207c67541",
          "url": "https://github.com/lemtoc-labs/nova/commit/cdf4195d7b0498d32fa7c4483c181a15522cb866"
        },
        "date": 1783683913670,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.16,
            "range": "1.97 ms",
            "unit": "ms",
            "extra": "median: 29.16 ms\nmin: 27.51 ms\nmax: 36.73 ms\nstddev: 1.97 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.29,
            "range": "1.98 ms",
            "unit": "ms",
            "extra": "median: 29.29 ms\nmin: 27.65 ms\nmax: 36.91 ms\nstddev: 1.98 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.56,
            "range": "0.02 ms",
            "unit": "ms",
            "extra": "median: 0.56 ms\nmin: 0.5 ms\nmax: 0.59 ms\nstddev: 0.02 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.59,
            "range": "0.21 ms",
            "unit": "ms",
            "extra": "median: 0.59 ms\nmin: 0.16 ms\nmax: 0.84 ms\nstddev: 0.21 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 21.14,
            "range": "1.07 ms",
            "unit": "ms",
            "extra": "median: 21.14 ms\nmin: 20.2 ms\nmax: 24.96 ms\nstddev: 1.07 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "8766502c1b4cc88f67b6d491c234cdd57a640fdb",
          "message": "feat(config): warn about unknown keys and segment tables (#55)\n\n- Capture ignored Serde field paths as warnings while preserving forward-compatible parsing and source spans\n- Flag unknown segment configuration tables without treating diagnostics as effective config changes\n- Cover nova check output, worker warning deduplication, dynamic maps, and inapplicable known keys\n\nCo-authored-by: Codex GPT-5.6 Sol Ultra <noreply@openai.com>",
          "timestamp": "2026-07-10T21:03:47+09:00",
          "tree_id": "cbccd2abc375d6546ce752ab8e865aa0e66bdbc0",
          "url": "https://github.com/lemtoc-labs/nova/commit/8766502c1b4cc88f67b6d491c234cdd57a640fdb"
        },
        "date": 1783685077144,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 30,
            "range": "2.63 ms",
            "unit": "ms",
            "extra": "median: 30 ms\nmin: 26.01 ms\nmax: 38.84 ms\nstddev: 2.63 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 30.14,
            "range": "2.65 ms",
            "unit": "ms",
            "extra": "median: 30.14 ms\nmin: 26.13 ms\nmax: 39.08 ms\nstddev: 2.65 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.59,
            "range": "0.05 ms",
            "unit": "ms",
            "extra": "median: 0.59 ms\nmin: 0.51 ms\nmax: 0.71 ms\nstddev: 0.05 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.43,
            "range": "0.22 ms",
            "unit": "ms",
            "extra": "median: 0.43 ms\nmin: 0.24 ms\nmax: 0.99 ms\nstddev: 0.22 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 22.43,
            "range": "1.83 ms",
            "unit": "ms",
            "extra": "median: 22.43 ms\nmin: 20.57 ms\nmax: 28.11 ms\nstddev: 1.83 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "9ce8bf33f62477b072b93dab9024c93e1251d955",
          "message": "chore(cargo): declare MSRV and release metadata (#56)\n\n- Declare Rust 1.88 and package discovery metadata\n- Keep release and dist builds on panic unwinding and guard worker isolation\n- Check all targets on Rust 1.88 in CI\n\nCo-authored-by: Codex GPT-5.6 Sol Ultra <noreply@openai.com>",
          "timestamp": "2026-07-10T21:18:44+09:00",
          "tree_id": "c03da3f3647116586296ad85ef2d03e2d05c84ce",
          "url": "https://github.com/lemtoc-labs/nova/commit/9ce8bf33f62477b072b93dab9024c93e1251d955"
        },
        "date": 1783685975006,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.79,
            "range": "2.95 ms",
            "unit": "ms",
            "extra": "median: 29.79 ms\nmin: 24.01 ms\nmax: 36.49 ms\nstddev: 2.95 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.95,
            "range": "2.96 ms",
            "unit": "ms",
            "extra": "median: 29.95 ms\nmin: 24.14 ms\nmax: 36.65 ms\nstddev: 2.96 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.58,
            "range": "0.01 ms",
            "unit": "ms",
            "extra": "median: 0.58 ms\nmin: 0.55 ms\nmax: 0.61 ms\nstddev: 0.01 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.58,
            "range": "0.17 ms",
            "unit": "ms",
            "extra": "median: 0.58 ms\nmin: 0.19 ms\nmax: 0.8 ms\nstddev: 0.17 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 21.57,
            "range": "1.4 ms",
            "unit": "ms",
            "extra": "median: 21.57 ms\nmin: 20.11 ms\nmax: 25.43 ms\nstddev: 1.4 ms\nruns: 16"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "83203852+lemtoc@users.noreply.github.com",
            "name": "k.suzuki",
            "username": "lemtoc"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "7a85cb123122d0c90e06270a4a9b4a10d21079ee",
          "message": "test(zsh): add interactive E2E harness (#57)\n\n* test(zsh): add interactive E2E harness\n\n- Drive zsh -f through zpty to cover startup, async git redraws, and vi keymaps\n- Verify SIGKILLed shells release their worker process and runtime directory\n- Run the harness in a dedicated CI job and document local usage\n\nCo-authored-by: Codex GPT-5.6 Sol Ultra <noreply@openai.com>\n\n* test(zsh): isolate E2E runtime directory\n\n- Clear inherited XDG_RUNTIME_DIR before starting the child shell\n- Keep worker runtime files under the harness temporary directory on hosted runners\n\nCo-authored-by: Codex GPT-5.6 Sol Ultra <noreply@openai.com>\n\n---------\n\nCo-authored-by: Codex GPT-5.6 Sol Ultra <noreply@openai.com>",
          "timestamp": "2026-07-10T21:55:28+09:00",
          "tree_id": "4343bd737a4457742c388576014ad998b3925789",
          "url": "https://github.com/lemtoc-labs/nova/commit/7a85cb123122d0c90e06270a4a9b4a10d21079ee"
        },
        "date": 1783688177738,
        "tool": "customSmallerIsBetter",
        "benches": [
          {
            "name": "first prompt lag",
            "value": 29.45,
            "range": "2.76 ms",
            "unit": "ms",
            "extra": "median: 29.45 ms\nmin: 28.41 ms\nmax: 40.6 ms\nstddev: 2.76 ms\nruns: 16"
          },
          {
            "name": "first command lag",
            "value": 29.59,
            "range": "2.77 ms",
            "unit": "ms",
            "extra": "median: 29.59 ms\nmin: 28.55 ms\nmax: 40.77 ms\nstddev: 2.77 ms\nruns: 16"
          },
          {
            "name": "command lag",
            "value": 0.57,
            "range": "0.02 ms",
            "unit": "ms",
            "extra": "median: 0.57 ms\nmin: 0.53 ms\nmax: 0.59 ms\nstddev: 0.02 ms\nruns: 16"
          },
          {
            "name": "input lag",
            "value": 0.72,
            "range": "0.2 ms",
            "unit": "ms",
            "extra": "median: 0.72 ms\nmin: 0.17 ms\nmax: 0.96 ms\nstddev: 0.2 ms\nruns: 16"
          },
          {
            "name": "exit time",
            "value": 21.35,
            "range": "0.6 ms",
            "unit": "ms",
            "extra": "median: 21.35 ms\nmin: 20.45 ms\nmax: 22.85 ms\nstddev: 0.6 ms\nruns: 16"
          }
        ]
      }
    ]
  }
}