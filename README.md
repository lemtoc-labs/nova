# Nova

[日本語版](docs/README.ja.md)

Nova is a fast, customizable zsh prompt renderer written in Rust.

It keeps the prompt's blocking path small and collects slower data, such as Git
status and runtime versions, through a per-shell worker process. zsh is the
primary shell target.

## Requirements

- zsh 5.8 or later
- A Nerd Font for the default icons
- Git, when Git segments are enabled

## Installation

Release artifacts are published from Git tags by cargo-dist. The commands below
work after the first GitHub Release is published.

### Shell Installer

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/lemtoc-labs/nova/releases/latest/download/nova-installer.sh | sh
```

### Homebrew

```sh
brew install lemtoc-labs/tap/nova
```

### Nix

Run without installing:

```sh
nix run github:lemtoc-labs/nova -- --help
```

Install into your profile:

```sh
nix profile install github:lemtoc-labs/nova
```

### mise

Use the GitHub backend:

```sh
mise use -g github:lemtoc-labs/nova@latest
```

Nova does not document the `ubi` backend because mise warns that it is
deprecated.

## Shell Setup

Add this to your `.zshrc` after `nova` is installed:

```zsh
eval "$(nova init zsh)"
```

For a temporary smoke test without touching your normal zsh configuration:

```sh
zsh -f
eval "$(nova init zsh)"
```

## Configuration

Most users should put their config here:

```text
~/.config/nova/nova.toml
```

If `XDG_CONFIG_HOME` is set, Nova uses that base directory instead:

```text
$XDG_CONFIG_HOME/nova/nova.toml
```

Config lookup order:

1. `--config PATH`
2. `$NOVA_CONFIG`
3. `$XDG_CONFIG_HOME/nova/nova.toml`
4. `$HOME/.config/nova/nova.toml`

Use `NOVA_CONFIG` when you want to load an explicit config file:

```sh
NOVA_CONFIG=/path/to/nova.toml
```

Start from the complete example:

```sh
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/nova"
cp examples/config.toml "${XDG_CONFIG_HOME:-$HOME/.config}/nova/nova.toml"
nova check --config "${XDG_CONFIG_HOME:-$HOME/.config}/nova/nova.toml"
```

Preview the prompt without starting an interactive shell:

```sh
nova prompt --format preview --cwd "$PWD" --cols "${COLUMNS:-80}" --duration-ms 2000 --time 11:16:42
```
