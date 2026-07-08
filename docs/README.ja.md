# Nova

[English](../README.md)

Nova は Rust で書かれた、高速でカスタマイズ可能な zsh prompt renderer
です。

<p align="center">
  <img src="assets/vhs/readme-prompt.gif" alt="Nova prompt demo">
</p>

Git status や runtime version のような遅い情報は per-shell worker process
で収集し、prompt の blocking path を小さく保ちます。主な対象 shell は zsh
です。

## Requirements

- zsh 5.8 以上
- default icon 表示用の Nerd Font
- Git segment を使う場合は Git

## Installation

Release artifact は Git tag から cargo-dist で publish されます。以下の
command は初回 GitHub Release の publish 後に使えます。

### Shell Installer

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/lemtoc-labs/nova/releases/latest/download/nova-installer.sh | sh
```

### Homebrew

```sh
brew install lemtoc-labs/tap/nova
```

### Nix

install せずに実行:

```sh
nix run github:lemtoc-labs/nova -- --help
```

profile に install:

```sh
nix profile install github:lemtoc-labs/nova
```

### mise

GitHub backend を使います:

```sh
mise use -g github:lemtoc-labs/nova@latest
```

`ubi` backend は mise 側で deprecated warning が出るため、Nova では案内しま
せん。

## Shell Setup

`nova` を install したあと、`.zshrc` に追加します:

```zsh
eval "$(nova init zsh)"
```

普段の zsh config を読み込まずに一時確認する場合:

```sh
zsh -f
eval "$(nova init zsh)"
```

## Configuration

ほとんどのユーザーはここに config を置けば十分です:

```text
~/.config/nova/config.toml
```

`XDG_CONFIG_HOME` を設定している場合は、その directory が base になります:

```text
$XDG_CONFIG_HOME/nova/config.toml
```

config の探索順:

1. `--config PATH`
2. `$NOVA_CONFIG`
3. `$XDG_CONFIG_HOME/nova/config.toml`
4. `$HOME/.config/nova/config.toml`

別の config file を明示する場合は `NOVA_CONFIG` に path を指定します:

```sh
NOVA_CONFIG=/path/to/config.toml
```

完全な example から始められます:

```sh
nova_config="${XDG_CONFIG_HOME:-$HOME/.config}/nova/config.toml"
mkdir -p "$(dirname "$nova_config")"
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/lemtoc-labs/nova/main/examples/config.toml \
  -o "$nova_config"
nova check --config "$nova_config"
```

interactive shell を起動せずに prompt を preview できます:

```sh
nova prompt --format preview --cwd "$PWD" --cols "${COLUMNS:-80}" --duration-ms 2000 --time 11:16:42
```
