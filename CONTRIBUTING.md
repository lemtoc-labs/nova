# Contributing

## Development Environment

Use direnv or Nix:

```sh
direnv allow
# or
nix develop
```

Cargo can also install Nova directly from source when you intentionally want a
development build:

```sh
cargo install --git https://github.com/lemtoc-labs/nova
```

## Checks

Run these before opening a pull request:

```sh
cargo-fmt --check
cargo-clippy --all-targets -- -D warnings
cargo test
cargo build
```

Validate the example config:

```sh
cargo run -- check --config examples/config.toml
```

Validate release distribution metadata:

```sh
dist plan
nix build .#nova --no-link
```

## Benchmarks

Build the release binary first:

```sh
cargo build --release
```

Run the zsh benchmark:

```sh
scripts/bench-zsh
```

See [docs/performance.md](docs/performance.md) for benchmark interpretation and
dashboard details.

## Release

Releases are built by cargo-dist from version tags such as `v0.1.0`.

The release workflow publishes:

- shell installer: `nova-installer.sh`
- Homebrew formula: `lemtoc-labs/homebrew-tap`
- macOS and Linux archives for x86_64 and aarch64
- source archive and checksums

Before the first release, maintainers must create the
`lemtoc-labs/homebrew-tap` repository and add a `HOMEBREW_TAP_TOKEN` repository
secret with write access to that tap.

Regenerate the release workflow after cargo-dist configuration changes, then pin
GitHub Actions:

```sh
dist generate --mode ci --allow-dirty
pinact run .github/workflows/release.yml
```
